---
name: robot-behavior
description: 使用或实现 roplat_behavior 机器人驱动：在应用中调用已有驱动（Franka、JAKA、Hans、AUBO、rsbullet 等）的运动、控制、状态与模型能力，或为新机器人实现/审查驱动 trait 与 roplat 适配。与 roplat-development 配合组图；不用于自行操作真实机器人。
---

# roplat_behavior 驱动能力

roplat_behavior 用类型化能力 trait 描述机器人能做什么：`Robot`、`MoveTo<S>` / `MoveTraj<S>`、`ControlWith<S>` / `AsyncControlWith<S>`、模型映射与状态视图。roplat 适配由默认关闭的 `roplat` feature 启用。

本 `next` 分支对应源码 `roplat_behavior 0.9`，先核对项目 manifest、锁文件、源码提交与 [version.json](references/version.json)。技能目录名 `robot-behavior` 保留以兼容既有链接。0.9 项目先读[迁移与依赖路径](references/migration-0.9.md)；截至 2026-10-27，五个行为/算法/描述包（含 `roplat_description`）与消息包尚未发布，必须使用匹配源码。已发布 `robot_behavior 0.8.1` / `roplat 0.5.1` 项目使用 [`main` 技能](https://github.com/Robot-Exp-Platform/roplat-skills/tree/main)，不要直接套用本分支的新采样 API。

0.9 的契约包仍需 nightly（`async_trait_bounds`）；各包已不再使用 `generic_const_exprs`（nightly-2026-10-20 的新 trait solver 不支持它），自己的 crate 也不要再开启。控制器、运动学和轨迹分别来自 `roplat_control`、`roplat_kinematics`、`roplat_trajectory`，不再从契约包导入。

## 按意图选择

| 用户意图 | 先读 |
|---|---|
| 用已有驱动写应用、示例、有界控制会话、Roplat 集成 | [use-robot-driver.md](references/use-robot-driver.md) |
| 接入新机器人、重构驱动、补 trait、审查实现语义 | [implement-robot-driver.md](references/implement-robot-driver.md) |
| 原生异步会话、并发控制节律、嵌套 Tokio runtime 排错 | [native-async-control.md](references/native-async-control.md) |
| 夹爪、力/力矩、IMU、相机；URDF 模型、限位与正运动学 | [robot-behavior-overview.md](references/robot-behavior-overview.md) 的“末端与传感器契约”“机器人描述” |
| 抓取与取放规划、抓取成功判据、命名位姿与末端工具、深度图与点云及凹形物体进场景、整段运动检查、桌面物体分割、机械臂自身点滤波、棋盘格检测与手眼标定、笛卡尔轨迹跟随（含仿真）、路径平滑、运行诊断，规划场景的 Rerun 显示，以及 Robotiq、ATI、WitMotion、SpaceMouse、大寰夹爪驱动（2026-10-30、10-31 加入 main，不在冻结集中） | [manipulation-perception-diagnostics.md](references/manipulation-perception-diagnostics.md) |

两者都先读 [robot-behavior-overview.md](references/robot-behavior-overview.md)：当前 trait、类型与语义速查。

## 与 roplat 的分工

- 设备能力、控制会话契约、驱动实现：本技能。
- 图怎么连、节律如何分层、对象在哪创建：[roplat-development](../roplat-development/SKILL.md)。
- 在 System 中使用 `&mut robot >> control.resources.0; control >> |..| {..};`。`MotionNode` 的 `Resources = (R,)`、Input 为目标、Output 为 `RobotResult<()>`，写 `&mut robot >> motion.resources.0; goal >> motion; motion?;`。控制节律的 Input/Output 均为 `()`，资源可在失败、停止后继续借用，无返回边或回收钩子。
- System 内创建的机器人由该 System 启停，设备类型需实现 `roplat::Lifecycle`（连接/断开）；作为参数传入则由调用方管理。

## 必须遵守

- 应用优先导入 `roplat_behavior::behavior::*`（根路径导入如 `roplat_behavior::JointPositionControl` 同样可用），驱动实现导入 `roplat_behavior::driver::*`。
- `Continue((command, done))` 发送有效指令；`Break(())` 不带算法指令，由驱动执行协议收尾。不要编造零指令填补失败或停止的周期，`Break` 也不是急停。
- 驱动不支持的能力返回明确错误，不静默改用其他运动模式。
- 编译、mock 或仿真通过不等于真机验证。技能不授权运行真机示例、上传数据或发布代码；运行连接设备的程序需要用户明确要求且环境安全。
- 源码与本技能冲突时，以用户实际依赖的 roplat_behavior 源码（crates.io 包或本地检出）及其 crate 文档为准。
