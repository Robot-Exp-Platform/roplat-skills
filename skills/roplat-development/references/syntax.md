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
| `source.output.1 >> sink.input.0;` | 元组字段投影与目的端口装配，编号从0开始 |
| `source.output.field >> sink;` | 按实际命名字段取值；Rust检查字段类型 |
| `source >> rhythm >> \|sample\| sample >> node;` | 父Input进入整次drive，sample是每轮Yield |
| `rhythm >> { node; };` | 不绑定Yield，每轮Feed=() |
| `child.output >> next;` | child整次drive的业务Output，默认传播失败/停止 |
| `child.outcome >> handler;` | 完整Execution交给父图显式处理 |
| `fallible.output? >> next;` | 节点业务Result显式提升错误 |

目的端整体输入和拆分端口不能混用；拆分端口须连续、无重复。节点单实例同层执行一次：`a >> b; a >> c;` 是扇出，不是调用 a 两次；不会隐式 Clone 非 Copy 输出。对需要复制/共享的数据明确设计其类型或节点输出。

## Rust 与 DSL 分区

同一 System/周期/match arm：普通 Rust 头段 → 连续图 → 普通 Rust 尾段。不能 `a >> b; reset(); b >> c;`。对象创建、配置放头段；状态检查与公开返回值放尾段。要返回 Rust 字段，用 `Ok(object.field)`；裸路径/字段可能按 DSL 端点解析。

不要把普通 `let n = 3; n >> sink;` 当常量边；n 通常被当成节点。将值存到 Source 节点或使用已绑定 Yield/match值。`if`/`loop` 内不会递归寻找 DSL，不能用普通 Rust 控制流包裹图后假定宏会调度。

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
