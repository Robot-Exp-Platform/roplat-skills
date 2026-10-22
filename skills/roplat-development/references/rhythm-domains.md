# 多层节律域：先确认四个边界

| 值 | 产生者 → 消费者 | 次数 |
|---|---|---|
| Rhythm::Input | 父图 → 一次 `drive` | 每个完整 drive 一次 |
| Rhythm::Yield | 节律 → 周期域 | 每轮一次 |
| Rhythm::Feed | 周期域 → 节律 | 每个成功周期一次 |
| Rhythm::Output | 完成的 drive → 父图 | 完整 drive 结束后一次 |

`Output` 不一定是最后的 `Feed`。节律可以累计所有 Feed、接收设备反馈，并返回不同类型的总结。父图遇到子节律后，等待它**结束当前完整 drive**，才得到子输出继续依赖链；不是子节律跑一次 tick 后父层就继续。

```rust
source >> outer >> |outer_yield| {
    outer_yield >> middle >> |middle_yield| {
        middle_yield >> inner >> |inner_yield| {
            inner_yield >> controller
        }
    }
};
outer >> sink;
```

以上是片段，各对象须有匹配类型。三处尾表达式无分号：controller Output 是 inner Feed，inner 完整 Output 是 middle Feed，middle 完整 Output 是 outer Feed。末尾的外层 `};` 结束父图语句，**不会改变最内层 controller 的 Feed**。如果在 inner 块里写 `controller;`，该 inner Feed 就是 `()`。

不要再写一份 inner 域来“再次调用”同一个实例。一个实例只有一个调用位置，跨作用域也一样（只有同一 match 的互斥分支可以共用）；重复周期由 Rhythm 产生，需要多处调用就建多个实例。

## 生命周期与存活范围

```rust
let mut controller = Controller::new();
let mut rhythm = make_rhythm();
rhythm >> |sample| {
    let mut transient = Temporary::new();
    sample >> controller >> transient;
};
```

controller 由外层创建：外层一次 init，多个周期复用状态，结束后一次 shutdown。transient 在每轮创建并由该周期启用/关闭，离开本轮后销毁。创建位置不会自动清空外部对象状态，也不存在“进入子域就重置”的默认动作。

如果 controller 是 System 函数的参数，当前 System 只使用它；创建者负责 hooks。这与参数按本体还是按 `&mut` 传入没有矛盾：宏会为普通同任务完整 join 借用互不重叠的对象，为 spawn 与拥有型域 `move |…|`（hard 节律只能由它进入）的捕获传播拥有需求；`Rhythm::drive` 的 N 可以是本体与可变借用的混合元组。节律代码把 N 当作不透明状态，不检查 N 的内部类型、地址或布局。

## 普通捕获值不等于图中的 N

节点/节律的捕获由宏做静态所有权规划；任意普通 Rust 配置值仍服从生成闭包的 FnMut 和 async move 捕获规则。多层回调重复运行时，不能从外层捕获的同一个 Arc 本体在每轮直接移走。例程将 Trace 作为 `&Trace` 参数传入：它活到本次 System 完整返回，同任务子域借用它，每轮新建的 local 对象显式 clone 自己的句柄。不要为消除错误加 unsafe 或强行要求所有对象 Clone；涉及 spawn 时应重新确认独立任务的拥有与 'static 需求。

## 多个设备持续运行

每个机器人通常维护自己的控制 Rhythm，算法放在各自周期域内。交换外部信息使用旁路；Latest 接收方采样当时最新值，不等所有接收方确认同一帧。

如果把“永不结束”的 child drive 放在 outer 的依赖链里，outer 就不会越过它。需要独立推进的控制域，应在父图作为没有依赖边的兄弟域组织，并为它们定义合作式停止；不能靠嵌套无限循环实现并行。

当前设计以实物机器人为准；仿真器闭环/加速语义尚待单独校订，不从 Iterator 或 SysTimer 推出通用仿真契约。

## 可执行证据

[control_layers/src/main.rs](../assets/control_layers/src/main.rs) 含三个有限 FoldRhythm，每层两轮：一次 outer drive 调用 middle 两次、inner 四次、worker 八次。每层加一个 1000 偏置，故完整 Output 和某轮 Feed 明显不同。它还重复调用整个 System，断言外部对象状态保留，并在最内层创建周期局部节点验证每轮启用/关闭。

机器人场景的分层（任务层分发目标 × 设备控制节律）、单层运动节点和多设备兄弟域见 [application-design.md](application-design.md) 与 [robot_apps](../assets/robot_apps/src/)。
