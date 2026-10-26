# 验证、版本核对与排错

本 `next` 分支验证当前源码组合：核心提交、行为库0.9提交及版本见 [version.json](version.json)。发布依赖验证在 [`main`](https://github.com/Robot-Exp-Platform/roplat-skills/tree/main)，不能因两者核心版本号同为0.5.1而混用。0.4的资源返回边、ResourceSlot与reclaim_input不适用于当前图语法。

## 可运行例程

```sh
python3 <skill目录>/scripts/verify_examples.py \
  --roplat /path/to/roplat --behavior /path/to/roplat_behavior --offline
```

需要 Python 3.11 及以上与 nightly。参数指向两个仓库根；脚本在临时副本的应用根 patch 核心、消息和四个行为/算法包，核对 manifest 包名及 Cargo 实际选用的来源，再运行断言与 `cargo clippy -- -D warnings`。源码模式重新解析锁文件并记录确切提交、dirty状态和依赖来源，不把 registry 回退误报为源码验证。没有缓存时去掉 `--offline`。

默认验证全部三个例程；`--only robot_apps` 或 `--only behavior_current` 可限制范围。锁文件保留已验证的解析组合，源码 patch 的版本/依赖变化需重新审阅解析结果。可用 `CARGO_TARGET_DIR` 复用构建缓存，证据与项目副本仍独立生成。路径布局见[迁移说明](../../robot-behavior/references/migration-0.9.md)。

| 例程 | 内容 | 断言 |
|---|---|---|
| [control_layers](../assets/control_layers/src/main.rs) | 三层有限节律、整次 drive Output、跨周期保留的对象状态、周期局部生命周期、失败后的关闭顺序 | 输出、调用次数与 init/shutdown 顺序 |
| [robot_apps](../assets/robot_apps/src/main.rs) `sequence_*` | 同一航点任务的三种结构：两层任务×控制、运动节点、由 System 创建并启停设备 | 到达位置与连接状态 |
| [plan_hard.rs](../assets/robot_apps/src/plan_hard.rs) | 阻塞线程的规划器放进 hard 节律，控制环同时保持节拍，经 `Latest` 交接 | 控制环最长周期间隔小于求解时间的一半（去掉 `.hard()` 后等于整个求解时间） |
| [task_events.rs](../assets/robot_apps/src/task_events.rs) | 操作员线程经事件通道发指令；守卫做限位检查，图内 `if` 用碰撞检查节点的结论，`retry` 重试驱动故障，信号合并刷新请求 | 拒绝与跳过的目标、重试次数、最终位置 |
| [dual_arm.rs](../assets/robot_apps/src/dual_arm.rs) | 两台设备各自的控制节律，经 `Latest` 旁路跟随 | 跟随臂保持偏移 |
| [behavior_current](../assets/behavior_current/src/main.rs) | 0.9 拆包、DH 映射、位姿/采样、纯数据控制器与轨迹、模拟模型选择、借用资源的 MotionNode | FK、有效位、控制结果、模型选择与最终关节位置 |

这些是特定负载的有限程序：不能推广为任意 DSL 写法都被支持，也不代表真实设备、实时性能或跨平台验证。

## 查看静态图

核对结构（尤其是 AI 写出的图）时画出来：`cargo install cargo-roplat` 后运行 `cargo roplat graph -p <包> [--all-targets] [--system <函数>] [--view flow|schedule] [--format svg|png|pdf|dot]`，需要 Graphviz（`ROPLAT_GRAPHVIZ_DOT` 可指定 `dot`）。它用独立 target 目录以 `roplat/graph` feature 编译，图由 System 宏展开时写出，与实际编译一致；输出在 `<target>/roplat-graph/<包>/<crate>/<函数>.<视角>.<格式>`。`flow` 是书写的图（端口、节律域、match、`.resources.N` 资源借用），`schedule` 是规约后的 Seq/Par/Spawn 树和切出的通道。旁路通信、内联 `system!{}` 不画；同一 crate 内同名 System 会覆盖。应用也可直接开启 `roplat = { features = ["graph"] }`，库不要开启（feature 会合并）。

## 常见误用

| 现象 | 首先检查 |
|---|---|
| node只有一个名字却期望调用多次 | 一个实例只有一个调用位置（跨作用域也一样）；重复由Rhythm产生，或定义不同实例 |
| `... is called both in ... and in ...` / `reading ... .output in this tail would call it a second time` | 同一实例在外层与内层（或两个都会运行的域）各调用了一次；改读域的 `.output`，或建两个实例；资源在调用所在的作用域绑定 |
| `resource ... is used by ... and ... with no data or .trigger dependency` | 同一资源的两个使用之间补数据边或 `a >> b.trigger;`；节律结果写 `child.outcome >> next.trigger;` |
| `a hard rhythm cannot be entered by a borrowing domain` | hard 节律的域写成 `move \|…\| { … }`；域内对象须 `Send + 'static`，只以引用到达时改为由创建层以本体传入 |
| `#[hard]` 绑定属性报错 | 0.5 改为 hard 节律：`let mut step = roplat::rhythm::once().hard();`，`goal >> step >> move \|goal\| { goal >> plan; plan };` |
| 守卫读取 `.output` 报错 | 守卫只读模式绑定和普通值；节点算出的条件用图内 `if` 或先输出可匹配的值 |
| 事件 / 信号节律的域一直不结束 | 还有触发器存活；触发器若由同一 System 内的节点持有，System 永远不会结束，见语法参考“内置节律” |
| `fire_blocking` panic | 在 Tokio 运行时内调用了它；节点、周期和 hard 节律里用 `try_fire` 或 `fire_async().await` |
| child后面的节点不运行 | child当前drive是否真的结束，是否无限Rhythm；Output不能早于drive返回 |
| Feed类型不匹配 | 周期尾表达式是否误加分号；child Output和child每轮Feed是否混淆 |
| 外部节点没初始化 | System参数只使用，创建层必须管理hooks |
| 每轮状态消失 | 是否把状态对象创建在周期内 |
| 普通Result导致输入类型不对 | 业务Result默认是数据；明确需要提升时用 `node? >> next` 或独立 `node.1?;` |
| 停止后再次drive直接Stopped | ExecutionContext粘性；新独立执行使用新context |
| 借用加入spawn后报错 | 跨任务需要Send/'static和拥有对象，不强行unsafe延长引用 |
| 节点开始被当成普通Rust运算 | 图被Rust语句分隔或放在 loop/for 里；回到连续DSL，中段计算用 `roplat::node::map` |
| 裸数值被当成Node | 头段值或参数写成 `&value >> x`；Yield/match 绑定可直接连 |
| 多个来源连到同一目标报错 | 写 `.input.0`、`.input.1`；语句顺序不决定元组位置 |
| 旧 `&mut arm >> user` 或归还边报错 | 改为 `&mut arm >> user.resources.0`；资源不放在 Input/Output |
| 本层创建的设备报缺 `Lifecycle` | 借出本层创建的资源时创建层负责启停；为设备实现连接/断开，或作为参数传入 |
| `.resources.N` 的数量或类型不匹配 | 对照 Node/Rhythm 的 Resources 类型元组，逐个连接连续槽号 |
| 自定义 Node 报 `Sync` 约束 | `Node` 要求 `Send + Sync`，持有设备的节点需要设备类型满足它 |

## 交付标准

修改后运行相关包的测试和 Clippy，按影响范围决定是否做完整验证。核心或驱动接口层面的改动先与用户讨论目的、利弊和兼容性；技能本身不授权这类重构。报告性能时写明版本、机器、负载、采样方式和失败项，不从展开代码更短或 Future 更小直接推断更快。消息边界的限制（bool 等字段的接收值不校验）见[通信说明](communication-and-languages.md)，不能据此宣称可以安全接收任意外部字节。有限测试通过不代表所有路径安全、完整或达到稳定发行标准。

维护者可用额外 `--patch CRATE=PATH` 验证特定源码，路径指向 crate 目录。验证器必须确认实际采用该路径，未使用或版本不兼容的显式 patch 判失败。源码模式不加 `--locked`；升级后的锁文件应复验后再纳入本分支。
