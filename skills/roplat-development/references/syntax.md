# 推荐的真实 System 语法

本文说明 roplat 0.5 System 的共同写法，片段可组合，节点名不是关键字。`next` 的类型与函数签名以 [version.json](version.json) 指定源码及该项目的 `cargo doc --open` 为准；docs.rs 的 0.5.1 文档对应发布包，不包含全部开发接口。可运行程序在本技能的 [assets](../assets/)。

## 入口与基本图

```rust
#[roplat::system]
async fn run(mut source: Source, mut sink: Sink) -> roplat::RoplatResult<()> {
    source >> sink;
    Ok(())
}
```

外部参数不会自动 init/shutdown。手写 Rust Node 只需实现 Lifecycle 和 Node，`#[node]` 不是加入 System 的必要条件。用户项目直接依赖 Tokio，宏生成代码使用 `tokio::` 路径。普通命名的 System 是 async 函数；名为 main 时宏可补 Tokio runtime。需要配置时按顺序 `#[roplat::system]` 再 `#[tokio::main(flavor = "current_thread")]`。

| 写法 | 意义 |
|---|---|
| `source >> next >> sink;` | 静态依赖链 |
| `node;` | 独立节点，未提供输入时 Input=() |
| `source.1 >> sink.input.0;` | 输出元组字段投影与目的端口装配，编号从0开始 |
| `source.field >> sink;` | 按实际命名字段取值；Rust检查字段类型 |
| `source >> rhythm >> \|sample\| sample >> node;` | 父Input进入整次drive，sample是每轮Yield |
| `rhythm >> { node; };` | 不绑定Yield，每轮Feed=() |
| `child >> next;` | child整次drive的业务Output，默认传播失败/停止 |
| `child.outcome >> handler;` | 完整Execution交给父图显式处理 |
| `fallible? >> next;`、`fallible.1?;` | 节点业务Result显式提升错误 |
| `&gain >> law.input.1;` | 头段值或函数参数的副本（`(gain).clone()`） |
| `&mut arm >> user.resources.0;` | 本次调用专用的可变资源借用，见下节 |

图语句的源端省略 `.output`（`node.output.x` 与 `node.x` 等价）；普通 Rust 尾段里裸名字是对象本身，引用输出必须写 `node.output`。多个来源连到同一目标时必须写 `.input.N`；`x >> b.trigger` 只排序不传值。

目的端整体输入和拆分端口不能混用；拆分端口须连续、无重复。单实例在所在域的每次执行中只调用一次：`a >> b; a >> c;` 是扇出，不是调用 a 两次；不会隐式 Clone 非 Copy 输出。对需要复制/共享的数据明确设计其类型或节点输出。

图中段不能写普通 Rust 运算。小的中段计算用闭包节点 `roplat::node::map(|value: f64| value * 2.0)`：它是普通节点（`Resources = ()`，生命周期钩子为空），闭包可以是 `FnMut` 并跨调用保留状态，也接受 `.input.N` 元组输入；闭包里的状态在图外读不到，需要资源、生命周期或可读状态时写具名节点类型。

## 资源：独立的 `.resources.N` 借用通道

```rust
&mut arm >> motion.resources.0;
goal >> motion;                 // Input = Target，Output = RobotResult<()>
motion?;                       // 业务失败先提升为框架错误
motion >> control.trigger;     // 同一资源的使用顺序由依赖明确连接
&mut arm >> control.resources.0;
control >> |(state, dt)| { /* 算法 */ servo };
control.outcome >> record;      // Execution<()>，可直接处理本次会话失败
```

- Node/Rhythm 声明 `Resources = ()`、`(Arm,)` 或 `(Arm, Camera)`。核心 `ResourceSet` 提供相应的 `()`、`(&mut Arm,)`、`(&mut Arm, &mut Camera)`，借用生命周期与 `process`/完整 `drive` 相同。不是 `&mut (Arm, Camera)`，也不能把映射改为按值资源。
- 多资源分别连接 `.resources.0`、`.resources.1`；编号连续且不重复，同一调用不能两次独占借用同一本体。目前支持最多 16 个扁平槽。单资源也必须写 `.resources.0`。
- 资源链交叉的复杂 DAG 由规约器补边与同 join 借用移交处理，`unsupported resource dependency shape` 只作防御性诊断；绑定上的 `#[hard]` 已改为 hard 节律（见下节）。不自行加锁或串行整个图。
- 相同条件的重复 match 会合并为一次图调用；在两次 match 之间插入的同一资源使用须与合并后的 match 建立依赖，否则按无依赖使用报错。应把相关使用放入同一个域，或用独立命名的 match 表达不同调用。
- 资源留在拥有状态里，业务 Input/Output 不携带它。删除的 `ResourceSlot`、`reclaim_input` 和 `user >> &mut arm` 不应出现在新程序中。
- 同一资源的任意两个使用之间写出数据或 `.trigger` 依赖（如 `reach >> settle.trigger;`）。使用只按依赖（含传递）排序，与绑定书写顺序无关；无依赖的两个使用编译报错，节律结果用 `.outcome` 处理时写 `child.outcome >> next.trigger;`。独立资源按图依赖并发。业务错误 `motion?` 与资源借用无关，不需要“归还后再检查”。借用允许正常 Rust 本体替换，不承诺对象身份不变。
- 业务检查与触发边分开写为 `motion?; motion >> control.trigger;`。crates.io 发布版 roplat 0.5.1（来源 `cfee536c79c4760ae7579367e27a8dd8bd3221f4`）的失败验证中，合并写法 `motion? >> control.trigger;` 没有传播业务错误，后续控制仍会执行。`next` 也采用分写；这一发布版负例不证明核心源码 `78fd47e` 的组合写法具有相同行为。
- 本层创建的资源由本层 Lifecycle 启停；参数由调用方管理。内层域可借用外层资源；尾段可调用资源方法。外部按值传入、由调用方管理的资源可 `Ok(arm)` 返回；本层创建并由 Lifecycle 管理的资源需留下执行关闭钩子，尾段返回业务状态快照，不能提前移走本体。节律持有资源期间，其周期域不能同时使用同一资源。
- Rhythm 的 `.outcome` 可直接隔离该 drive 的失败；要把一组节点作为一段隔离，用 `once()` 子域，逐段收集业务结果用 `into_collect_rhythm()`（见内置节律）。仅在后续任务独立于失败段时继续。
- Node 业务错误用业务错误类型加 `Into<RoplatError>`，或 `RoplatError::NodeProcessing(..)`；设备 `RobotException` 已有转换。

## 内置节律

| 构造 | 每周期 Yield | 一次 drive 的结束与 Output | 典型用途 |
|---|---|---|---|
| `iter.into_rhythm()`（`roplat::rhythm::IntoRhythm`） | 迭代器的下一项 | 迭代器耗尽；Output `()`。位置跨 drive 保留 | 航点、任务步骤 |
| `iter.into_collect_rhythm()` | 同上 | 同上；Output 是本次各周期 Feed 的 `Vec` | 逐段收集结果 |
| `iter.with_period(dt)`（`roplat::rhythm::WithPeriod`） | 下一项，按 `dt` 定时 | 迭代器耗尽 | 定频采样、脚本化输入 |
| `CountRhythm::new(n)`；`create_count_rhythm(n)` → `(CountHandle, CountRhythm)` | 计数 | 跑满 n 个周期 | 有限次循环；工厂函数需先解构，句柄可调整计数 |
| `once::<I, F>()` | Input 本身 | 运行一个周期；Output 是该周期 Feed | 把一段图包成可隔离、可 hard 的域 |
| `retry::<I, O, E>(n)` | `(input, attempt)`，attempt 从 0 起 | 某周期 Feed 为 `Ok`，或本次 drive 用满 n 次；Output 是最后一次 Feed（`Result<O, E>`，全失败仍是业务数据，`retry.output?` 才升级为错误）。`retry.attempts()` 是累计次数 | 重试一个任务步骤 |
| `feedback(n, finished)` | 首周期为 Input，之后为上一周期 Feed | `finished(&feed)` 成立或用满 n 个周期；Output 是最后一次 Feed | 状态机：节点读上一结果决定下一指令 |
| `create_event_channel::<T>(cap)` → `(EventTrigger<T>, EventRhythm<T>)` | 每个事件一份 | 全部触发器 drop 且队列取空 | 操作员指令、外部请求 |
| `create_signal()` → `(SignalTrigger, SignalRhythm)` | `()` | 全部触发器 drop 且没有待处理的触发 | 只需知道“发生过”：刷新、唤醒 |

`once`、`retry`、`feedback`、事件与信号节律在周期停止或失败时结束 drive 并请求停止。`once()` 每次 drive 都运行一次，不能用会耗尽的 `(0..1).into_rhythm()` 替代。

发送事件：`EventTrigger::try_fire` 不等待，任何上下文都能用，队列满或接收端已结束时把事件交回（`FireError::Full` / `Closed`）；`fire_async().await` 在异步代码里等待空位，前提是接收域能同时推进；`fire_blocking` 让线程睡眠等空位，只用于 Tokio 运行时之外的线程（UI、设备回调线程），在节点、周期或 hard 节律里调用会 panic。`SignalTrigger::fire` 不等待、不分配、不 panic；下一个周期之前的多次触发合并为一个周期，不携带数据，需要每个事件或载荷时用事件通道。

触发器决定这两种节律何时结束：只要还有一个触发器存活，节律就继续等待。触发器交给同一 System 里另一个节点持有时，该节点要到 System 结束才 drop，而 System 又在等这个节律结束，于是永远不会结束。所以要么让触发器留在 System 之外（操作员线程、另一个程序部分），发送完 drop；要么用停止上下文（`#[roplat::system(context = ...)]`）结束整个 System。完整写法见 [task_events.rs](../assets/robot_apps/src/task_events.rs)。

## 重计算与实时回路：hard 节律

完整范例见 [plan_hard.rs](../assets/robot_apps/src/plan_hard.rs)：规划在 hard 节律的线程上求解，机械臂控制环在同一 System 里保持 2 ms 节拍，二者经 `Latest` 交接。

```rust
let mut infer = roplat::rhythm::once::<Frame, Detections>().hard();
let mut control = ControlRhythm::new().hard();
camera >> |frame| {
    frame >> infer >> move |frame| { frame >> detector; detector };
    infer.output >> tracker;
};
&mut robot >> control.resources.0;
control >> move |(obs, dt)| { obs >> law; law };
```

- `rhythm.hard()`（`roplat::rhythm::IntoHard`）让每次 drive 在该节律自己的线程与运行时上执行：不阻塞同任务兄弟和其他域，也不被别处的同步代码拖慢。用于规划、推理、长时间的外语调用和需要隔离的控制回路；每次进入约多几微秒，不用于轻节点。
- hard 节律只能由拥有型域 `move |…| { … }` 进入：域内节点以本体传入，节律借用的资源也以本体移入，drive 结束随结果交回，之后的使用者看到修改。被移动的对象须 `Send + 'static`；对象只以引用到达时编译报错。写成 `|…| { … }` 时，编译错误指向该域并提示 `move`。
- 下游照常等待 hard 域的结果。可能超出周期时限、只需最新结果的计算放进自己的节律，经 `Latest` 旁路交付，不放进周期图。
- 生命周期仍归创建层：线程在第一次 drive 时启动，由 `on_shutdown` 关闭并等待。旧写法 `#[hard] let mut node = ...;` 会得到改写提示。
- 调用方 future 被丢弃（`timeout`、`select!`）不会停下线程：该次 drive 被放弃并收到停止请求，其结果与对象在 hard 线程丢弃，下一次 drive 只返回自己的结果。线程 panic 后该节律不可再用，后续 drive 立即返回框架错误。

## Rust 与 DSL 分区

同一 System/周期/match arm：普通 Rust 头段 → 连续图 → 普通 Rust 尾段。不能 `a >> b; reset(); b >> c;`。对象创建、配置放头段；状态检查与公开返回值放尾段。要返回 Rust 字段，用 `Ok(object.field)`；裸路径/字段可能按 DSL 端点解析。

普通 `let n = 3; n >> sink;` 中裸名字 n 被当成节点；头段值或函数参数写成值副本 `&n >> sink;`，同一名字在一个图中只有一种角色（副本、资源或节点/节律）。`loop`/`for`/`while` 不会被当作图，不能用它们包裹图来重复调度，重复由节律产生。`if` 只有在分支含图语句或条件读取 `.output` / `.outcome` 时才是图内 `if`（见条件分支），头段和尾段的普通 `if` 仍是 Rust。

周期和 match 头段不允许会跳出当前归还边界的 `?`/`return`。需要失败的准备工作放 Lifecycle::on_init、上层，或当前支持的显式结果域。闭包/async内部Rust错误传播属于它们自己的真实边界。

## 条件分支

```rust
match chooser.output {
    Some(value) => { value >> work; }
    None => { fallback; }
}

match command {
    Command::Goto(target) if within_limits(&target) => {
        target >> clear;
        if clear.output { target >> motion; } else { target >> blocked; }
    }
    Command::Goto(target) => { target >> out_of_reach; }
    Command::Dwell(time) => { time >> dwell; }
}

let speed = if urgent.output { boost } else { cruise };
speed >> limiter;
```

- 只执行选中的分支。被匹配表达式或 `if` 条件是直接端点（`node.output`、`.outcome`、Yield 或模式绑定）时建立图依赖；复杂表达式如 `match compute(x)` 原样作为 Rust 求值，不从中推导依赖。
- 模式绑定可作为数据源；同一 match 的互斥分支可以使用同一节点和同一资源，资源在使用它的分支里绑定。
- 守卫 `pattern if cond` 原样保留为 Rust，只读本分支绑定和普通值，不建立图依赖；守卫读取 `node.output` 会在宏展开时报错。条件需要节点计算（例如持有障碍物模型的碰撞检查）时，让节点输出 bool 再用图内 `if`，或输出适合匹配的值再 match。
- 图内 `if` 是 bool 的 match：`else` 可省略，可写 `else if` 链，`if let P = e { … } else { … }` 等价于 `match e { P => …, _ => … }`。条件写成 let 链会报错，改写为 match。
- 分支结果用 `let name = match … { … };` 或 `let name = if … { … } else { … };` 汇合，各分支尾值类型一致，再连 `name >> sink`。目前不支持 `let mut`、类型标注、解构和 let-else；先绑定整体，再用字段端口连接。
- 分支内可用端点 `?`；跨分支 `return` 被拒绝。没有结果绑定、被匹配表达式相同的旧 match 会合并为一次调用（见资源一节），新代码优先写命名结果。
- 完整用法见 [task_events.rs](../assets/robot_apps/src/task_events.rs)：守卫做静态限位检查，图内 `if` 用碰撞检查节点的结论。

## 文件入口与限制

`#[roplat::system(file = "architecture.yaml", context = context)]` 使用文件前端和共享停止域；文件路径相对用户crate，YAML能力小于Rust DSL，不能据Rust三层例程推断YAML支持同等嵌套。未知属性被宏拒绝；`stop_check` 不是可用属性。

`system_item!` 用于已有async上下文内联块，失败/停止按普通值边界处理并可能panic；需要清楚的Execution边界时优先具名 `#[system]` 函数。spawn必须满足Send/'static；普通分支优先让System自动同任务join，不为获得并发随意加spawn。0.5.1 只在根作用域 spawn，节律周期与 match 内一律 join；要隔离重计算或实时回路，用上文的 hard 节律，不寻找 spawn 写法；`#[spawn]` 不是语句属性。
