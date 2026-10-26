# 0.8 发布包与 0.9 源码迁移

本页适用于从 `robot_behavior 0.8.x` 迁到 `roplat_behavior 0.9` 的项目。2026-10-26 核实时，已发布组合是 `roplat 0.5.1` + `robot_behavior 0.8.1`；`roplat_behavior 0.9`、三个算法包与 `roplat_msgs` 尚未发布；2026-10-27 新增的 `roplat_description` 同样未发布。版本相同也不能证明源码与发布包相同，尤其核心 main 的录制/重放和消息接口已继续演进。

## 仓库、包与依赖

`Robot-Exp-Platform/robot_behavior` 仓库已改名为 `Robot-Exp-Platform/roplat_behavior`。当前布局是同级 `roplat/`、`roplat_behavior/`、`drives/`，不再使用 `drives/robot_behavior` 子模块。行为仓本身是 workspace，契约包在 `roplat_behavior/roplat_behavior/`；把仓库根目录直接当成 crate 路径会失败。

| 旧导入或位置 | 0.9 导入或位置 |
|---|---|
| `robot_behavior::behavior::*` / `driver::*` | `roplat_behavior::behavior::*` / `driver::*` |
| `robot_behavior::controller::*`、实时优先级工具 | `roplat_control::*` |
| DH、FK/IK、动力学 helper、`combine_array` | `roplat_kinematics::*` |
| `utils::path_generate::*` | `roplat_trajectory::path_generate::*` |
| `utils::trajectory::*`、COPP 规划器 | `roplat_trajectory::*` |
| `Pose`、采样、`StateView`、`LoadState` | `roplat_msgs`，契约包继续重新导出 |
| Python 模块 / C++ 头与命名空间 | `roplat_behavior` / `roplat_behavior.hpp` / `roplat_behavior` |

未发布组合在**最终应用 workspace 根**声明补丁。下面路径仅示意应用位于同级目录；按实际布局调整，不能照抄本机绝对路径。依赖仓内部的 `[patch]` 不会自动传给应用。

```toml
[dependencies]
roplat = "0.6.0-rc.1"
roplat_behavior = { version = "0.9.0", features = ["roplat"] }
# 只添加实际使用的算法包
roplat_kinematics = "0.9.0"
roplat_control = "0.9.0"
roplat_trajectory = "0.9.0"

[patch.crates-io]
roplat = { path = "../roplat/roplat" }
roplat_msgs = { path = "../roplat/roplat_msgs" }
roplat_behavior = { path = "../roplat_behavior/roplat_behavior" }
roplat_kinematics = { path = "../roplat_behavior/roplat_kinematics" }
roplat_control = { path = "../roplat_behavior/roplat_control" }
roplat_trajectory = { path = "../roplat_behavior/roplat_trajectory" }
```

若直接使用 `roplat_msgs` 类型，另加 `roplat_msgs = { version = "0.6.0-rc.1", features = ["nalgebra", "serde"] }`；仅 patch 不会引入依赖。`roplat` feature 只在需要图适配器时启用。源码验证要记录两个仓库的完整提交、dirty 状态与最终锁文件。

## 必须迁移的写法

| 0.8 | 0.9 |
|---|---|
| `Pose::Euler(p, rpy)` | `Pose::from_euler(p, rpy)` |
| `Pose::Quat(iso)` | `Pose::from_isometry(iso)` |
| `Pose::Homo(m)` / `AxisAngle(p, a, t)` / `Position(p)` | `from_homo(m)` / `from_axis_angle(p, a, t)` / `from_position(p)` |
| 对 Pose enum 做 `match` | 用 `homo()`、`position()`、`quat_array()` 等访问器 |
| `sample.q`、`sample.dq` | `sample.q()`、`sample.dq()`（仍返回 Option） |
| `JointSample { q: Some(q), ..Default::default() }` | `JointSample::EMPTY.with_q(q)` |
| `sample.q = Some(q)` / `None` | `sample.set_q(Some(q))` / `set_q(None)` |
| `spatial.vel` / `acc` | `spatial.twist()` / `accel()` |
| `SpatialSample { vel: Some(v), … }` | `SpatialSample::EMPTY.with_twist(v.into())` |
| `LoadState { m, x, i }` | `LoadState { mass, center_of_mass, inertia }` |
| nalgebra `Twist` / `Wrench` 向量 | `roplat_kinematics::SpatialVec`；同名新类型是消息结构体 |

`Pose` 的新布局是位置加四元数（x, y, z, w）；`Vec<f64>::from(pose)` 统一输出七个数。旧 JSON enum 表示可读，新输出为结构体。`Pose / Pose` 运算已移除。不要据 Rust 字段变更修改 Python `LoadState(m, x, i)` 的旧参数名，FFI 兼容层与 Rust 布局分别核对。

`JointSample::default()` 不含有效字段；`SpatialSample::default()` 和 `ContactSample::default()` 仍含全部有效默认字段。需要缺测时显式用 `EMPTY`，不要将缺测填成零。meas / cmd / des 的区别继续保留。

DH helper 已不再自动实现 `SpaceMap`。在准确 DH 参数实现旁添加 `roplat_kinematics::dh_kinematics!(MyArm, N)`；需要逆解映射时用 `dh_kinematics!(MyArm, N, inverse)`。`ForwardKinematics` 等能力 trait 仍在契约包。

资源连接、控制会话和退出语义未改变：`&mut robot >> control.resources.0`、`MotionNode` 的目标 Input 与结果 Output，以及阻塞会话和原生异步会话的区分继续适用。

## 验证入口

旧0.8发布例程在 [`main` 分支](https://github.com/Robot-Exp-Platform/roplat-skills/tree/main/skills/roplat-development/assets/robot_apps)。本 `next` 的 `robot_apps` 已迁移到0.9，迁移要点另有独立例程 [behavior_current](../../roplat-development/assets/behavior_current/src/main.rs)，实际断言 Pose/采样、四个拆分包、DH 模型映射、模拟模型选择及 `MotionNode` 资源借用。运行方式见 [验证说明](../../roplat-development/references/validation.md)。编译或纯数据验证不代表真机与后端模型加载已验证。
