# 应用设计：从机器人任务到 System

本篇讲“写成什么样”，语法细节见 [syntax.md](syntax.md)，语义边界见其余参考。完整可运行范例在 [robot_apps](../assets/robot_apps/src/)：仿真臂实现与真实驱动相同的 `robot_behavior` 能力 trait，换成真实驱动类型后应用文件不变。

## 1. 设计哲学如何落到代码

| 原则 | 写法上的含义 |
|---|---|
| Rhythm 拥有循环，System 拥有拓扑，Node 只拥有计算 | 应用里不出现 `loop`/`for` 驱动节点或反复调用 System；重复由节律产生，顺序由图边表达 |
| 机器人决定自己的节拍 | 每台设备一个控制节律；周期开始拿到状态，域内算完直接交付指令，不经中间队列 |
| 多层节律表达任务结构 | 外层节律分发任务/目标，内层是设备控制节律；一层只做一件事 |
| 进程内静态 | 节点、节律、连接都在编译期可见；不在运行时注册、查找或动态拼图 |
| 跨设备只靠旁路 | 不同设备的域之间没有图边，交换的是 `Latest` 最新值；不追求同步快照或锁步 |
| 创建位置即生命周期 | 跨周期保留的对象在外层创建；每段/每周期的状态在该域头段创建，域结束即销毁 |
| 设备是资源，不是数据 | 机器人用 `&mut arm >> user.resources.0` 为单次调用提供可变借用；不把它当作端口数据在节律间传递 |
| 业务结果是数据 | 只在确实要中断执行时用端点 `?` 传播；需要父层处理时用 `.outcome` |

## 2. 设计步骤

1. **列设备与节拍**：每台设备用哪个控制通道（关节位置、力矩、末端速度……），节拍由设备驱动决定。
2. **划分节律层**：任务层（目标序列、事件、外部指令）在外，设备控制节律在内；能用一个封装好的运动节点完成的段落不必开内层节律。
3. **拆节点**：每个节点一个职责（参考生成、伺服律、限幅、共享状态），输入输出用领域类型；节点不关心自己被哪个节律驱动。
4. **定创建位置**：问“这个状态跨任务段保留吗？”保留的放外层；每段重新开始的（如本段轨迹参考）放该段的头段。
5. **定跨域通信**：同一周期内的因果数据走图边；不同设备/不同节拍之间走旁路。
6. **写 System**：头段按设备分组创建对象 → 一段连续的图 → 尾段只组织返回值。验证、断言、打印放在调用方。

## 3. 推荐模式

### 3.1 设备控制节律：一个周期的计算

```rust
&mut arm >> control.resources.0;
control >> |(state, dt)| {
    state >> path.input.0;
    dt >> path.input.1;
    state >> servo.input.0;
    path >> servo.input.1;
    servo
};

```

- `.resources.0` 把机器人借给控制节律，借用覆盖整次会话；业务 Input/Output 为 `()`，失败和停止结束后仍保留资源。
- Yield `(state, dt)` 直接解构，同一个 `state` 可连到多个消费者（Copy 值扇出）。
- 尾表达式无分号：`servo` 的 `(command, done)` 就是本周期 Feed。`done = true` 发出最后一条指令后正常结束会话。
- 超周期、停发、协议收尾由设备节律自己决定，应用不在图里模拟这些策略。

### 3.2 两层节律：任务层分发目标，控制层驱动机器人

任务节律每个周期给出一个目标，周期内把机器人借给控制节律、用完归还（[sequence_control.rs](../assets/robot_apps/src/sequence_control.rs)）：

```rust
tasks >> |goal| {
    let mut path = MinJerk::towards(goal, SEGMENT); // 每段新的参考
    &mut arm >> control.resources.0;
    control >> |(state, dt)| { /* 同 3.1 */ servo };

};
Ok(arm)
```

`servo`、`control` 在外层创建，跨段保留；`path` 是段内状态，在任务周期头段创建。`arm` 是参数时由调用方管理连接；在 System 头段 `let mut arm = ...` 创建时由本 System 连接与断开（[sequence_owned.rs](../assets/robot_apps/src/sequence_owned.rs)，设备类型需实现 `Lifecycle`）。尾段的 `arm` 仍是设备本体，可调用方法取得业务状态。仅外部按值传入的资源可按示例 `Ok(arm)` 返回；本层创建并管理 Lifecycle 的资源须留到关闭钩子执行，返回状态快照而非移走本体。

### 3.3 一层节律：用 MotionNode 代替控制层

驱动已能完成点到点运动时，用 `robot_behavior::roplat::MotionNode` 代替内层节律（[sequence_motion.rs](../assets/robot_apps/src/sequence_motion.rs)）：

```rust
tasks >> |goal| {
    &mut arm >> motion.resources.0;
    goal >> motion;
    motion?;              // 显式提升运动错误
};
```

选择依据：需要逐周期参与计算（反馈控制、在线修正、与其他设备耦合）用两层；只需要“到达目标”用一层。同一周期内可以先用 `MotionNode` 到位、再借给控制节律做一段会话；两次借出之间写 `motion >> control.trigger;` 建立依赖：同一资源的使用只按数据与 `.trigger` 依赖排序，与绑定书写顺序无关，无依赖的两次使用编译报错。`motion?;` 失败时，整个图以错误退出，后面的控制会话不会开始；资源仍由拥有层持有。

### 3.3.1 按段收集结果

`into_rhythm()` 的迭代器节律 Feed 与 Output 都是 `()`，只负责分发。每段要产生结果、整次汇总返回时，写一个小的任务节律：Yield 是任务，Feed 是本段结果，Output 是结果列表（写法参考 [control_layers](../assets/control_layers/src/main.rs) 的有限节律，停止与错误时 `request_stop` 并原样返回）。周期尾用普通 Rust 读本段对象状态（如 `measure.cycles()`）作为 Feed 是合法的：尾段在本周期对象关闭前执行。

### 3.4 多设备：兄弟控制域 + 旁路

（[dual_arm.rs](../assets/robot_apps/src/dual_arm.rs)）两条控制链在同一 System 中没有依赖边，是兄弟域，各按自己的节拍并发推进：

```rust
&mut leader >> leader_control.resources.0;
leader_control >> |(state, dt)| { ...; leader_servo >> share };

&mut follower >> follower_control.resources.0;
follower_control >> |(state, _dt)| {
    state >> mirror;
    state >> follower_servo.input.0;
    mirror >> follower_servo.input.1;
    follower_servo
};

```

`Share` 发布本设备最新指令并原样透传，`Mirror` 读取对方最新值。读者每次拿当时最新的版本，可能跳版本，这是有意的。`Latest::<T>::new(n)` 的 `n` 是订阅者数量，`subscriber(i)` 取第 i 个。非机械臂设备（传送带、传感器）若没有自己的驱动节律，可用有限周期节律 `(0..ticks).with_period(period)`（`roplat::rhythm::WithPeriod`）驱动其节点。

**兄弟域如何结束**：每个域必须有自己的结束条件，并考虑对方先结束的情况。若跟随者只在“对方报告完成”时结束，而生产者可能在报告前耗尽节拍，跟随者会永远等待：让生产者在最后一周期发布结束标志，或给跟随者加上自己的上限。

**跟随者何时结束**：旁路传递的是“对方整个任务已完成”，不是“对方本段完成”。单段时二者相同；主设备有多段任务时，在任务节律结束后用一个发布完成标志的节点（如 `tasks >> announce`），跟随者以此判定结束。先读完成标志再读指令，可保证看到完成时拿到的是最后一条指令。

### 3.5 不再需要的构件

旧版本的 `Handoff`、把机器人放进 Yield/Feed 的 `TaskSequence`、资源返回边及 `reclaim_input` 不属于 0.5 的借用协议。已有节律若本来就内部拥有设备，可保留该所有权并声明 `Resources = ()`；不要为了统一外观强行外置设备。使用旧版本仓库时按对应版本写法，不混用。

## 4. 代码风格

- **一个 System 一个场景**：文件开头一两句说明这个应用的结构意图；System 函数保持在一屏内。
- **支撑与应用分离**：设备、节点实现放在独立模块；System 文件只做创建和连线。
- **命名用领域词**：`path`、`servo`、`mirror`、`leader_control`，不用 `node1`、`worker`、`a`。变量名表达在系统中的角色，类型名表达能力。
- **头段按设备分组**：同一设备的节律与其算法节点相邻创建，组间空行；跨设备共享的通道放最前。
- **注释写意图与选择理由**（为什么这里每段新建、为什么用旁路），不复述语法。
- **类型别名固定设备组合**：`type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;`
- **节点内不放测试代码**：不在 `process` 里 `assert!`、记录调用轨迹或计数；行为验证放在调用方或测试里。
- **尾段只组织返回值**：外部拥有参数可 `Ok(arm)`，本层管理资源返回状态快照，业务端口可 `Ok(child.output)`；复杂处理先 `let value = node.output;` 再交给普通 Rust。
- **图语句省略 `.output`**：写 `path >> servo.input.1`、`motion? >> next`；`.output` 只在普通 Rust 尾段引用输出时使用。

## 5. 反模式

| 写法 | 问题 | 改为 |
|---|---|---|
| 在 System 外 `for goal in goals { arm = move_to(arm, goal).await?; }` | 循环脱离节律与生命周期管理；中途失败丢失设备 | 外层任务节律分发目标（3.2/3.3） |
| 把机器人当数据在节律 Yield/Feed 里传来传去，或写 `Handoff` 节点 | 失败路径丢失设备，结构难读 | `.resources.N` 可变借用 |
| 将资源混入 Input/Output 或写旧归还边 | 0.5 不支持旧借出协议 | 声明 Resources 并连接 `.resources.N`，业务错误正常 `?` |
| 手写 `loop { node.process(x).await }` 代替 System | 失去静态结构、归还与停止语义 | 节律 + System 图 |
| 把所有算法塞进一个大节点 | 无法复用、图里看不出结构 | 按职责拆节点，用边表达数据流 |
| 用全局 `Arc<Mutex<_>>` 在节点间传值 | 绕开图的因果顺序与所有权 | 同周期用图边；跨域用 Latest/SPSC |
| 两台设备共用一个节律或用图边连接两个控制域 | 一台设备的节拍拖住另一台 | 各自控制节律 + 旁路 |
| 段内状态（本段轨迹）创建在外层并手动 reset | 依赖隐式重置，易遗漏 | 在任务周期头段创建 |
| 节点内断言/计数/打印用于验证 | 产物像测试而非应用 | 调用方断言最终结果 |
| 在图中段插普通 Rust、在 `if` 里写 `>>` | 宏拒绝或不当作图 | 头段准备、尾段处理；分支用 DSL match |
