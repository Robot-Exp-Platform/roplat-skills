---
name: robot-behavior
description: 使用或实现 robot_behavior 机器人驱动：在应用中调用已有驱动（Franka、JAKA、Hans、AUBO、rsbullet 等）的运动、控制、状态与模型能力，或为新机器人实现/审查驱动 trait 与 roplat 适配。与 roplat-development 配合组图；不用于自行操作真实机器人。
---

# robot_behavior 驱动能力

robot_behavior 用类型化的能力 trait 描述机器人能做什么：`Robot`（生命周期与原生状态）、`MoveTo<S>`/`MoveTraj<S>`（运动空间）、`ControlWith<S>`/`AsyncControlWith<S>`（控制通道）、模型映射与状态视图。roplat 适配在默认关闭的 `roplat` feature 下。先核对用户依赖的 robot_behavior 与 roplat 版本：本技能对应 robot_behavior `0.8.1` 与 roplat `0.5.1`（见 [version.json](references/version.json)），两者都已发布到 crates.io；robot_behavior 使用 nightly 特性（`async_trait_bounds`、`generic_const_exprs`），需要 nightly 工具链。

## 按意图选择

| 用户意图 | 先读 |
|---|---|
| 用已有驱动写应用、示例、有界控制会话、Roplat 集成 | [use-robot-driver.md](references/use-robot-driver.md) |
| 接入新机器人、重构驱动、补 trait、审查实现语义 | [implement-robot-driver.md](references/implement-robot-driver.md) |
| 原生异步会话、并发控制节律、嵌套 Tokio runtime 排错 | [native-async-control.md](references/native-async-control.md) |

两者都先读 [robot-behavior-overview.md](references/robot-behavior-overview.md)：当前 trait、类型与语义速查。

## 与 roplat 的分工

- 设备能力、控制会话契约、驱动实现：本技能。
- 图怎么连、节律如何分层、对象在哪创建：[roplat-development](../roplat-development/SKILL.md)。
- 在 System 中使用 `&mut robot >> control.resources.0; control >> |..| {..};`。`MotionNode` 的 `Resources = (R,)`、Input 为目标、Output 为 `RobotResult<()>`，写 `&mut robot >> motion.resources.0; goal >> motion; motion?;`。控制节律的 Input/Output 均为 `()`，资源可在失败、停止后继续借用，无返回边或回收钩子。
- System 内创建的机器人由该 System 启停，设备类型需实现 `roplat::Lifecycle`（连接/断开）；作为参数传入则由调用方管理。

## 必须遵守

- 应用优先导入 `robot_behavior::behavior::*`（根路径导入如 `robot_behavior::JointPositionControl` 同样可用），驱动实现导入 `robot_behavior::driver::*`。
- `Continue((command, done))` 发送有效指令；`Break(())` 不带算法指令，由驱动执行协议收尾。不要编造零指令填补失败或停止的周期，`Break` 也不是急停。
- 驱动不支持的能力返回明确错误，不静默改用其他运动模式。
- 编译、mock 或仿真通过不等于真机验证。技能不授权运行真机示例、上传数据或发布代码；运行连接设备的程序需要用户明确要求且环境安全。
- 源码与本技能冲突时，以用户实际依赖的 robot_behavior 源码（crates.io 包或本地检出）及其 crate 文档为准。
