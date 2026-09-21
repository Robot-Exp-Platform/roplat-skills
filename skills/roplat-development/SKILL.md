---
name: roplat-development
description: 使用 roplat 的 Node、Rhythm 和 System 宏编写、修改或排查机器人程序；尤其适用于多层节律域、端口连接、对象生命周期、停止与错误传播。用于开发代码，不用于自行操作真实机器人。
---

# roplat 开发

目标是产出使用当前真实 API、保持静态图语义并可编译验证的程序。先核对技能内 [version.json](references/version.json) 与用户仓库版本；已有项目的依赖和当前源码优先。整个技能目录可单独复制使用。不要用旧 RFC、历史性能原型或旧生成代码猜测支持的语法。

## 首要选择

**用 `#[roplat::system]` 表达应用的节点连接与节律组织。** 在 `impl Node` 中实现 `process`，在自定义 `impl Rhythm` 中实现 `drive`，但不要把应用层 System 改写成手工 `node.process(...).await` 调用链。手写展开代码仅适合用户明确要求底层诊断或独立性能对照；它不能替代用户要的 System 程序。遇到不支持的图形或语法，先定位限制，说明最小合法写法或需讨论的核心能力。

一套对象、一个静态图位置，不是每条语句调用一次。图边定义依赖；互不依赖分支可能并发。宏决定安全的借用/拥有规划，应用代码无需自己写归还槽或模拟调度器。

## 按任务阅读

- 初次写 System、端口或条件分支：读 [syntax.md](references/syntax.md)，按其中真实语法写，不发明 `->`、`if` 图或 `stop_check` 配置。
- **涉及任何嵌套节律、跨周期状态或子节律输出：先读 [rhythm-domains.md](references/rhythm-domains.md)，再看 [完整三层宏例程](assets/control_layers/src/main.rs)。** 不要仅由“循环”二字推断各层关系。
- 创建节点、控制停止、处理错误、自定义节律：读 [execution-lifecycle.md](references/execution-lifecycle.md)。
- 交换外部状态、选通信类型、接入 C++/Python 节点：读 [communication-and-languages.md](references/communication-and-languages.md)。
- 编译问题、版本核对、验收：读 [validation.md](references/validation.md)。

## 编码前固定四件事

1. 对象由哪一层创建，哪一层负责启用和关闭；哪些对象跨周期保留。
2. 每个节点的 Input/Output；每个节律的 Input/Output 和 Yield/Feed。
3. 哪些边是图中数据依赖，哪些是独立节律采样外部最新状态。
4. 哪些错误只是业务值，哪些显式提升为框架失败，何处需要 `.outcome` 隔离处理。

普通 Rust 语句只放在当前图的头段或尾段，不能插进两段 DSL 之间。生命周期与错误处理不能依靠丢弃在途 Future 完成。

## 交付前核对

实际运行相应 Rust 项目；断言输出、调用次数、跨周期状态和需要的生命周期/错误行为，不能只查看打印日志。多层例程可通过 `scripts/verify_examples.py --roplat <核心仓库> --offline` 验证。只报告真实完成的检查，标明硬件、远程服务或不受支持路径。不得因技能存在而扩大用户授权去控制设备、上传数据或发布代码。
