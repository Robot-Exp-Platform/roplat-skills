# 验证、版本核对与排错

先确定用户 roplat 版本。当前技能对应核心 `0.5.0` 与 robot_behavior `0.8.0` 的独立 Resources 借用通道；版本和验证来源见 [version.json](version.json)。历史评估保留原提交与旧契约，不能算作当前版本复验。核对当前核心 `docs/system-syntax.md`、`docs/execution-protocol.md` 与 `docs/lifecycle.md`，不混用 0.4 的资源返回边。已撤回的 `stop_check` 实验不是当前功能。

## 可运行例程

```sh
python3 <skill目录>/scripts/verify_examples.py --roplat <核心仓库> --drives <drives仓库> --offline
```

脚本复制assets及固定Cargo.lock到新临时目录，写入本地路径依赖并执行cargo run/clippy --locked，不在原仓生成target，不使用机器人/网络，也不安装技能。无离线依赖缓存时可去掉--offline，先按用户环境判断是否允许下载。不给 `--drives` 时只验证 control_layers，并提示跳过 robot_apps。

[control_layers](../assets/control_layers/src/main.rs)是语义例程：3层有限节律输出、复用对象状态、周期局部生命周期和失败后的关闭顺序均由断言验证。[robot_apps](../assets/robot_apps/src/main.rs)是应用风格范例：仿真臂上的三种航点结构与双臂旁路，由调用方断言到达位置。不能把这些特定负载的成功推广为任意DSL语法都支持，也不代表真实设备验证。

## 查看静态图

核对结构（尤其是 AI 写出的图）时画出来：`cargo roplat graph -p <包> [--all-targets] [--system <函数>] [--view flow|schedule] [--format svg|png|pdf|dot]`，需要 Graphviz（`ROPLAT_GRAPHVIZ_DOT` 可指定 `dot`）。它用独立 target 目录以 `roplat/graph` feature 编译，图由 System 宏展开时写出，与实际编译一致；输出在 `<target>/roplat-graph/<包>/<crate>/<函数>.<视角>.<格式>`。`flow` 是书写的图（端口、节律域、match、`.resources.N` 资源借用），`schedule` 是规约后的 Seq/Par/Spawn 树和切出的通道。旁路通信、内联 `system!{}` 不画；同一 crate 内同名 System 会覆盖。应用也可直接开启 `roplat = { features = ["graph"] }`，库不要开启（feature 会合并）。核心仓库 `examples/graph_gallery` 有 7 种结构的样例。

## 常见误用

| 现象 | 首先检查 |
|---|---|
| node只有一个名字却期望调用多次 | 同层一个调度位置；重复由Rhythm产生，或定义不同实例 |
| `... is called both in ... and in ...` / `reading ... .output in this tail would call it a second time` | 同一实例在外层与内层（或两个都会运行的域）各调用了一次；改读域的 `.output`，或建两个实例；资源在调用所在的作用域绑定 |
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
| 旧 `&mut arm >> user` 或归还边报错 | 改为 `&mut arm >> user.resources.0`；资源不放在 Input/Output |
| 本层创建的设备报缺 `Lifecycle` | 借出本层创建的资源时创建层负责启停；为设备实现连接/断开，或作为参数传入 |
| `.resources.N` 的数量或类型不匹配 | 对照 Node/Rhythm 的 Resources 类型元组，逐个连接连续槽号 |
| 自定义 Node 报 `Sync` 约束 | `Node` 要求 `Send + Sync`，持有设备的节点需要设备类型满足它 |

修改后运行相关包测试和Clippy，按影响范围选择完整验证。核心/驱动大的契约变更先讨论目的、利弊、兼容性和保留项；技能本身不授权这样的重构。对性能保留版本、机器、时钟分辨率、负载、采样和失败项，不能从展开代码短或Future小直接宣称更快。长期10%回退目标仍存在，本轮允许披露未达标项。消息边界已增加静态契约、padding 初始化编码和 Python 布局/类型检查，但 bool 等受限字段的接收值验证仍按作者决定延期；不能据此宣称可安全接受任意外部字节。当前 main 已包含 ReplayFeedSource 与类型化比较器；使用其他版本时仍须核对其公开 API。技能例程复验只覆盖下述 Rust/System/旁路负载，不替代核心 Replay 和外语边界的专项验证。有限测试通过不代表所有路径安全、完整或达到稳定发行标准。

例程保留独立锁文件；依赖变化使 `--locked` 失败时，审阅并重新验证锁文件后更新技能资产，不在交付脚本中静默解除锁定。
