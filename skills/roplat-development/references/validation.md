# 验证、版本核对与排错

先确定用户 roplat 版本。当前技能例程对应核心 `0.4.0`（DSL 修订：`.input.N`、`&value`、`&mut` 资源借出）与 robot_behavior `0.7.0`；实际复验的提交记录在 [version.json](version.json) 的 `core_commit`、`drives` 与 `verified_source_digest`，更早的验证保存在 `earlier_main_verification`、`historical_validation` 与 `previous_verification`。用户仓库仍是 0.3.0 时按其版本写法，不要混用。消息边界与 Replay 新能力已整合，来源见 `integrated_changes`；核心完整工程验收见 `c595393` 的 `docs/candidate/integration-validation.md`。当前定位是未公开发行的内部工程候选，保留已知限制和性能未达标项。用当前 `docs/system-syntax.md`、`docs/dsl-revision/README.md`、`docs/execution-protocol.md`、`docs/lifecycle.md` 和 `docs/static-ownership-planning.md` 核对契约；旧RFC及skill不是当前依据。候选的 `stop_check` 实验已撤回；错误提示优于猜测宏会忽略参数。

## 可运行例程

```sh
python3 <skill目录>/scripts/verify_examples.py --roplat <核心仓库> --drives <drives仓库> --offline
```

脚本复制assets及固定Cargo.lock到新临时目录，写入本地路径依赖并执行cargo run/clippy --locked，不在原仓生成target，不使用机器人/网络，也不安装技能。无离线依赖缓存时可去掉--offline，先按用户环境判断是否允许下载。不给 `--drives` 时只验证 control_layers，并提示跳过 robot_apps。

[control_layers](../assets/control_layers/src/main.rs)是语义例程：3层有限节律输出、复用对象状态、周期局部生命周期和失败后的关闭顺序均由断言验证。[robot_apps](../assets/robot_apps/src/main.rs)是应用风格范例：仿真臂上的三种航点结构与双臂旁路，由调用方断言到达位置。不能把这些特定负载的成功推广为任意DSL语法都支持，也不代表真实设备验证。

## 查看静态图

核对结构（尤其是 AI 写出的图）时画出来：`cargo roplat graph -p <包> [--all-targets] [--system <函数>] [--view flow|schedule] [--format svg|png|pdf|dot]`，需要 Graphviz（`ROPLAT_GRAPHVIZ_DOT` 可指定 `dot`）。它用独立 target 目录以 `roplat/graph` feature 编译，图由 System 宏展开时写出，与实际编译一致；输出在 `<target>/roplat-graph/<包>/<crate>/<函数>.<视角>.<格式>`。`flow` 是书写的图（端口、节律域、match、`&mut` 借出与归还），`schedule` 是规约后的 Seq/Par/Spawn 树和切出的通道。旁路通信、内联 `system!{}` 不画；同一 crate 内同名 System 会覆盖。应用也可直接开启 `roplat = { features = ["graph"] }`，库不要开启（feature 会合并）。核心仓库 `examples/graph_gallery` 有 7 种结构的样例。

## 常见误用

| 现象 | 首先检查 |
|---|---|
| node只有一个名字却期望调用多次 | 同层一个调度位置；重复由Rhythm产生，或定义不同实例 |
| child后面的节点不运行 | child当前drive是否真的结束，是否无限Rhythm；Output不能早于drive返回 |
| Feed类型不匹配 | 周期尾表达式是否误加分号；child Output和child每轮Feed是否混淆 |
| 外部节点没初始化 | System参数只使用，创建层必须管理hooks |
| 每轮状态消失 | 是否把状态对象创建在周期内 |
| 普通Result导致输入类型不对 | 业务Result默认是数据；明确需要提升时用 `node? >> next` 或独立 `node.1?;` |
| 停止后再次drive直接Stopped | ExecutionContext粘性；新独立执行使用新context |
| 借用加入spawn后报错 | 跨任务需要Send/'static和拥有对象，不强行unsafe延长引用 |
| 节点开始被当成普通Rust运算 | 图被Rust语句分隔或放在普通if/loop里；回到连续DSL |
| 裸数值被当成Node | 头段值或参数写成 `&value >> x`；Yield/match 绑定可直接连 |
| 多个来源连到同一目标报错 | 写 `.input.0`、`.input.1`；语句顺序不决定元组位置 |
| 借出资源报“未归还/再次借出” | 每个 `&mut arm >> user` 在同一图中要有 `... >> &mut arm`，下一次借出前先归还 |
| 本层创建的设备报缺 `Lifecycle` | 借出本层创建的资源时创建层负责启停；为设备实现连接/断开，或作为参数传入 |
| 尾段拿到 `ResourceSlot` 而非设备 | 借出过的名字在尾段是槽：直接调用方法，或 `into_inner()` 取出 |
| 自定义 Node 报 `Sync` 约束 | `Node` 要求 `Send + Sync`，持有设备的节点需要设备类型满足它 |

修改后运行相关包测试和Clippy，按影响范围选择完整验证。核心/驱动大的契约变更先讨论目的、利弊、兼容性和保留项；技能本身不授权这样的重构。对性能保留版本、机器、时钟分辨率、负载、采样和失败项，不能从展开代码短或Future小直接宣称更快。长期10%回退目标仍存在，本轮允许披露未达标项。消息边界已增加静态契约、padding 初始化编码和 Python 布局/类型检查，但 bool 等受限字段的接收值验证仍按作者决定延期；不能据此宣称可安全接受任意外部字节。当前 main 已包含 ReplayFeedSource 与类型化比较器；使用其他版本时仍须核对其公开 API。技能例程复验只覆盖下述 Rust/System/旁路负载，不替代核心 Replay 和外语边界的专项验证。有限测试通过不代表所有路径安全、完整或达到稳定发行标准。

例程独立锁文件分别选择 Tokio 1.52.1（control_layers）与 1.53.2（robot_apps），当前核心工作区锁文件为 1.48.0；它们是独立用户工程的兼容验证，不是同锁文件的核心回归测量。核心依赖变化使 `--locked` 失败时，应审阅并重新验证锁文件后更新技能资产，不要在交付脚本中静默解除锁定。
