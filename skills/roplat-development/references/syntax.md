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
| `&mut arm >> user;` / `user >> &mut arm;` | 借出资源本体 / 归还本体，见下节 |

图语句的源端省略 `.output`（`node.output.x` 与 `node.x` 等价）；普通 Rust 尾段里裸名字是对象本身，引用输出必须写 `node.output`。多个来源连到同一目标时必须写 `.input.N`；`x >> b.trigger` 只排序不传值。

目的端整体输入和拆分端口不能混用；拆分端口须连续、无重复。节点单实例同层执行一次：`a >> b; a >> c;` 是扇出，不是调用 a 两次；不会隐式 Clone 非 Copy 输出。对需要复制/共享的数据明确设计其类型或节点输出。

## 资源：`&mut` 借出与归还

设备本体不是节点，而是轮流交给使用者的资源。

```rust
&mut arm >> control >> |(state, dt)| { /* 算法 */ servo };   // 控制会话期间节律持有
control >> &mut arm;                                           // 整次 drive 的 Output 归还
&mut arm >> motion.input.0;                                    // 运动节点：(R, Target) -> (R, RobotResult<()>)
goal >> motion.input.1;
motion.0 >> &mut arm;
motion.1?;                                                     // 在本体归还后检查
```

- 同一资源可在一个图中多次使用，按书写顺序串行；每次借出必须在本图归还，宏会拒绝未归还、重复借出、`&mut arm.field` 以及把同名当节点使用。
- 本层 `let` 创建的资源由本层 `Lifecycle` 启停（设备类型需实现 Lifecycle）；入口函数参数也可借出，由调用方管理。内层域可借出外层资源。
- 尾段看到的是 `ResourceSlot`：可经 Deref 调用方法（`arm.joints()`），用 `arm.into_inner()` 取出返回。
- drive 未完成时节律经 `reclaim_input` 交还本体；使用者自己的独立 `user.x?;` 在归还后检查。可能在借出后、归还前以 `?` 离开并跳过归还的写法会被宏拒绝（诊断“`arm` is lent while `x` can leave the graph with `?` ...”），如归还要等 `user.1? >> verdict` 的下游；改为先归还、再独立检查。控制域内部的 `guard?`（持有本体的是节律）合法：失败后节律交还、创建层照常关闭。借出窗口内的节点不做停止准入检查，已开始的窗口先走完。但规约切出的图内通道在窗口内等待时仍响应停止，本体可能随作用域被 Drop：核心只保证唯一性，丢失即该路径失效，对象可重建；丢失前处理由资源类型的 `Drop` 负责。丢失后再借出同一资源得到框架错误 `RoplatError::Resource`（可用单次子域 `.outcome` 隔离），不会 panic。
- 借出给节律时归还使用其 Output（`control >> &mut arm`），因此同一图不能再读 `control.outcome`。“一段失败后继续下一段”的标准写法是用单次子域隔离这一段：

  ```rust
  goals >> |goal| {
      let mut segment = (0..1).into_rhythm();
      segment >> {
          &mut arm >> control >> |(state, dt)| { /* ... */ guard? };
          control >> &mut arm;
      };
      segment.outcome >> record;      // Execution<()>：本段成败，任务继续
  };
  ```

  多数设备任务仍应让失败中止整个任务；只在后续段与失败段无关时才继续。
- 节点判定的业务拒绝用自己的错误类型并实现 `Into<RoplatError>`，或用 `RoplatError::NodeProcessing(..)`；设备错误 `RobotException` 已可转为 `RoplatError`。
- 不再需要 `Handoff`、`TaskSequence` 一类“把机器人当数据传递”的适配节点。

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

`system_item!` 用于已有async上下文内联块，失败/停止按普通值边界处理并可能panic；需要清楚的Execution边界时优先具名 `#[system]` 函数。spawn必须满足Send/'static；普通分支优先让System自动同任务join，不为获得并发随意加spawn。高级spawn具体写法查该版本核心文档，不能把 `#[spawn]` 臆造为合法任意语句属性。
