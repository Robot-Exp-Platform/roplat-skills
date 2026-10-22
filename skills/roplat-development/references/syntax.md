# 推荐的真实 System 语法

完整权威参考位于核心仓库 `docs/system-syntax.md` 与 `examples/system_syntax/`。以下是可组合片段，节点名不是关键字。

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

## 资源：独立的 `.resources.N` 借用通道

```rust
&mut arm >> motion.resources.0;
goal >> motion;                 // Input = Target，Output = RobotResult<()>
motion?;
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
- 本层创建的资源由本层 Lifecycle 启停；参数由调用方管理。内层域可借用外层资源；尾段可调用资源方法。外部按值传入、由调用方管理的资源可 `Ok(arm)` 返回；本层创建并由 Lifecycle 管理的资源需留下执行关闭钩子，尾段返回业务状态快照，不能提前移走本体。节律持有资源期间，其周期域不能同时使用同一资源。
- Rhythm 的 `.outcome` 可直接隔离该 drive 的失败。要把一组节点作为一段隔离，可用 `once()` 子域；每次 drive 都运行一次，不能用会耗尽的 `(0..1).into_rhythm()` 替代。
- 逐段收集业务结果可用 `into_collect_rhythm()`，其 Output 是本次各周期 Feed 的 `Vec`。仅在后续任务独立于失败段时继续。
- Node 业务错误用业务错误类型加 `Into<RoplatError>`，或 `RoplatError::NodeProcessing(..)`；设备 `RobotException` 已有转换。

## 重计算与实时回路：hard 节律

```rust
let mut infer = roplat::rhythm::once::<Frame, Detections>().hard();
camera >> |frame| {
    frame >> infer >> move |frame| { frame >> detector; detector };
    infer.output >> tracker;
};
let mut control = ControlRhythm::new().hard();
&mut robot >> control.resources.0;
control >> move |(obs, dt)| { obs >> law; law };
```

- `rhythm.hard()`（`roplat::rhythm::IntoHard`）让每次 drive 在该节律自己的线程与运行时上执行：不阻塞同任务兄弟和其他域，也不被别处的同步代码拖慢。用于规划、推理、长时间的外语调用和需要隔离的控制回路；每次进入约多几微秒，不用于轻节点。
- hard 节律只能由拥有型域 `move |…| { … }` 进入：域内节点以本体传入，节律借用的资源也以本体移入，drive 结束随结果交回，之后的使用者看到修改。被移动的对象须 `Send + 'static`；对象只以引用到达时编译报错。写成 `|…| { … }` 时，编译错误指向该域并提示 `move`。
- 下游照常等待 hard 域的结果。可能超出周期时限、只需最新结果的计算放进自己的节律，经 `Latest` 旁路交付，不放进周期图。
- 生命周期仍归创建层：线程在第一次 drive 时启动，由 `on_shutdown` 关闭并等待。旧写法 `#[hard] let mut node = ...;` 会得到改写提示。
- 调用方 future 被丢弃（`timeout`、`select!`）不会停下线程：该次 drive 被放弃并收到停止请求，其结果与对象在 hard 线程丢弃，下一次 drive 只返回自己的结果。线程 panic 后该节律不可再用，后续 drive 立即返回框架错误。
- 节点里向其他域发事件：`EventTrigger::try_fire`（不等待，任何上下文）或 `fire_async().await`；`fire_blocking` 只用于 tokio 运行时之外的线程。只需通知“发生过”时用 `roplat::rhythm::create_signal()`，多次触发会合并为一个周期。

## Rust 与 DSL 分区

同一 System/周期/match arm：普通 Rust 头段 → 连续图 → 普通 Rust 尾段。不能 `a >> b; reset(); b >> c;`。对象创建、配置放头段；状态检查与公开返回值放尾段。要返回 Rust 字段，用 `Ok(object.field)`；裸路径/字段可能按 DSL 端点解析。

普通 `let n = 3; n >> sink;` 中裸名字 n 被当成节点；头段值或函数参数写成值副本 `&n >> sink;`，同一名字在一个图中只有一种角色（副本、资源或节点/节律）。`if`/`loop` 内不会递归寻找 DSL，不能用普通 Rust 控制流包裹图后假定宏会调度。

周期和 match 头段不允许会跳出当前归还边界的 `?`/`return`。需要失败的准备工作放 Lifecycle::on_init、上层，或当前支持的显式结果域。闭包/async内部Rust错误传播属于它们自己的真实边界。

## 条件分支

```rust
match chooser.output {
    Some(value) => { value >> work; }
    None => { fallback; }
}
```

只执行选中的分支；模式绑定可作为数据源，互斥分支可以使用同一对象。命名结果汇合采用现有 `let chosen = match ...`，各分支尾值类型一致，再将 `chosen >> sink`。需要结果/错误组合时先查看核心 `system_execution.rs` 测试，不发明 `if graph` 或任意分支回跳。

## 文件入口与限制

`#[roplat::system(file = "architecture.yaml", context = context)]` 使用文件前端和共享停止域；文件路径相对用户crate，YAML能力小于Rust DSL，不能据Rust三层例程推断YAML支持同等嵌套。未知属性被候选宏拒绝，`stop_check` 是已撤回试验，不可使用。

`system_item!` 用于已有async上下文内联块，失败/停止按普通值边界处理并可能panic；需要清楚的Execution边界时优先具名 `#[system]` 函数。spawn必须满足Send/'static；普通分支优先让System自动同任务join，不为获得并发随意加spawn。0.5.0 只在根作用域 spawn，节律周期与 match 内一律 join；要隔离重计算或实时回路，用上节的 hard 节律，不寻找 spawn 写法。高级spawn具体写法查该版本核心文档，不能把 `#[spawn]` 臆造为合法任意语句属性。
