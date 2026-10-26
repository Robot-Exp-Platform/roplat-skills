# roplat_behavior 概念速查

这是 0.9 源码的能力与语义速查。0.8 项目先按 [迁移说明](migration-0.9.md) 区分旧 API。

源码仓库为 `roplat_behavior`，契约 crate 在其 `roplat_behavior/` 子目录；控制器、运动学、轨迹分别在同仓的 `roplat_control/`、`roplat_kinematics/`、`roplat_trajectory/`。共享位姿和采样在同级核心仓库 `roplat/roplat_msgs/`。旧 `drives/robot_behavior` 位置不再是当前集成布局。

## 公开入口

`roplat_behavior/roplat_behavior/src/lib.rs` 暴露应用与驱动入口，控制器在独立包：

- `roplat_behavior::behavior::*`：应用用户入口。用于调用已有驱动。
- `roplat_behavior::driver::*`：驱动作者入口。额外导出 `MoveTo`、`MoveTraj`、`ControlWith` 等实现侧 trait。
- `roplat_control::*`：控制器闭包生成函数入口。

应用示例不要随手 `use roplat_behavior::driver::*`，否则容易把实现侧 trait 带入作用域并制造方法歧义。

## 根 trait 与能力束

- `Robot`
  - 根 trait。
  - 关联 `type State`。
  - 声明 `CONTROL_PERIOD`。
  - 必须实现 `version()` 和 `read_state(&mut self)`。
  - 生命周期方法有默认空实现：`init`、`shutdown`、`enable`、`disable`、`reset`、`stop`、`pause`、`resume`、`emergency_stop`、`clear_emergency_stop`。
  - `is_moving` 与 `waiting_for_finish` 仅在驱动有真实语义时覆盖。

- `Joints<N>`
  - 固定关节数量和关节限制。
  - 必须给出 `JOINT_MIN` / `JOINT_MAX`。
  - 可给出 `JOINT_DEFAULT`、`JOINT_PACKED`、速度、加速度、jerk、力矩和力矩变化率限制。
  - 单位使用 rad、rad/s、rad/s^2、rad/s^3、N*m、N*m/s。

- `EndPoint`
  - 笛卡尔末端限制。
  - 线速度/加速度/jerk 使用 m/s、m/s^2、m/s^3。
  - 角速度/加速度/jerk 使用 rad/s、rad/s^2、rad/s^3。

- `Arm<N>`
  - 机械臂能力束，不是所有机器人的根模型。
  - 组合 `Robot + Joints<N> + EndPoint + MoveTo<JointSpace<N>> + MoveTo<FlangeSpace>`。
  - 提供 `state()`、`set_load()`、关节/末端读取、限制读取和 `with_*` 调整接口。

- `MobileBase`、`Quadruped<N>`、`Humanoid<N>`
  - 面向移动底盘、四足、人形的能力束。
  - 不要把它们强行降级成 `Arm`。

## 状态模型

状态用 `StateView<T>` 表达三种语义：

```rust
pub struct StateView<T> {
    pub meas: T,
    pub cmd: T,
    pub des: T,
}
```

常用状态：

- `JointSample<N>`：固定布局与有效位掩码；用 `q()`、`dq()`、`ddq()`、`tau()`、`dtau()` 取得 `Option<[f64; N]>`。构造用 `JointSample::EMPTY.with_q(q).with_dq(dq)`，更新用 `set_q(Some(q))` / `set_q(None)`，不要直接读写旧 Option 字段。
- `JointState<N> = StateView<JointSample<N>>`。
- `SpatialSample`：用 `pose()`、`twist()`、`accel()`、`wrench()` 读取有效值；构造用 `EMPTY.with_pose(pose).with_twist(array.into())`。`ContactSample` 用 `is_active()`、`force()`、`point()`、`normal()`；`LoadState` 字段为 `mass`、`center_of_mass`、`inertia`。
- `BaseState = StateView<SpatialSample>`。
- `EndEffectorState = StateView<SpatialSample>`。
- `ArmState<N>`：
  - `joint: JointState<N>`
  - `flange: StateView<SpatialSample>`
  - `tcp: Option<StateView<SpatialSample>>`
  - `stiffness: Option<StateView<SpatialSample>>`
  - `load: Option<LoadState>`
- `MobileBaseState`、`QuadrupedState<N>`、`HumanoidState<N>` 用于非机械臂形态。

驱动转换状态时要尽量保留 measured / commanded / desired 的区别，不要只填 measured 后丢弃其余信息。

## Pose 与单位

`Pose` 是 `roplat_msgs` 的结构体，由契约包重新导出，字段为 `position: Vector3` 与 `orientation: Quaternion`（x, y, z, w）。使用 `Pose::from_euler`、`from_isometry`、`from_homo`、`from_axis_angle`、`from_position`，不再使用旧 enum 变体或模式匹配。访问器仍为 `euler()`、`quat()`、`quat_array()`、`homo()`、`axis_angle()`、`position()`。

`Twist` / `Wrench` 是消息结构体；需要旧 nalgebra 六维向量时使用 `roplat_kinematics::SpatialVec`。`JacobianModel::twist` 返回消息 `Twist`。

边界单位约定：

| 量 | 单位 |
|---|---|
| 关节角 | rad |
| 关节角速度 | rad/s |
| 关节角加速度 | rad/s^2 |
| 关节 jerk | rad/s^3 |
| 力矩 | N*m |
| 长度 | m |
| 线速度 | m/s |
| 空间速度 | `[vx, vy, vz, wx, wy, wz]` |
| 空间力/力矩 | `[fx, fy, fz, tx, ty, tz]` |
| 时间 | `std::time::Duration` |

厂商单位必须在驱动边界转换。

## Motion 语义

`MotionSpace<R>` 是 type-level marker，只定义 `type Target`。

常用 motion spaces：

- `JointSpace<N>` -> `[f64; N]`
- `FlangeSpace` -> `Pose`
- `TcpSpace` -> `Pose`
- `EndSpace` -> `Pose`
- `BasePoseSpace` -> `Pose`
- `BaseVelocitySpace` -> `[f64; 6]`
- `WholeBodyJointSpace<N>` / `WholeBodyVelocitySpace<N>` / `WholeBodyTorqueSpace<N>` -> `[f64; N]`
- `CenterOfMassSpace` -> `[f64; 3]`
- `GaitSpace` -> `GaitCommand`
- `FootSpace<LEG>` / `HandSpace<HAND>` -> `Pose`
- `Relative<S>` / `Inertial<S>` 包装空间坐标语义

`MoveTo<S>`：

- 驱动实现侧 trait。
- `move_to(&mut self, target)` 当前语义是阻塞，完成或失败后返回。
- `move_to_async` 返回惰性 Future；只有真实 async 后端才应覆盖。
- 默认 `move_to_async` 返回 unsupported error。

`MoveTraj<S>`：

- 驱动实现侧 trait。
- `move_traj(Vec<S::Target>)` 是基础轨迹入口。
- 当前 trait 要求实现 `move_path` 和 `move_waypoints`；不支持时要显式返回 `RobotException::UnprocessableInstructionError`。
- `move_traj_sync`、`move_path_sync`、`move_waypoints_sync` 默认调用对应方法后 `waiting_for_finish()`。

`Motion` 和 `MotionFile` 是 blanket user-facing facade，用户通过 turbofish 选择空间：

```rust
robot.move_to::<JointSpace<7>>(q)?;
robot.move_to::<FlangeSpace>(pose)?;
robot.move_traj::<JointSpace<7>>(traj)?;
robot.move_traj_from_file::<JointSpace<7>>("traj.json")?;
```

## Control 语义

`ControlSpace<R>` 是 type-level control channel，定义：

- `type Obs`
- `type Command`

内置 control spaces：

| Control space | Obs | Command |
|---|---|---|
| `JointPositionControl<N>` | `JointState<N>` | `[f64; N]` |
| `JointVelocityControl<N>` | `JointState<N>` | `[f64; N]` |
| `TorqueControl<N>` | `JointState<N>` | `[f64; N]` |
| `ArmTorqueControl<N>` | `ArmState<N>` | `[f64; N]` |
| `CartesianPoseControl<N>` | `ArmState<N>` | `Pose` |
| `CartesianVelocityControl<N>` | `ArmState<N>` | `[f64; 6]` |
| `BaseVelocityControl` | `BaseState` | `[f64; 6]` |
| `BalanceControl` | `BaseState` | `[f64; 6]` |

`ControlWith<S>`：

- 驱动实现规范入口 `control_with_flow`，它是阻塞控制 session。
- 回调签名：`FnMut(S::Obs, Duration) -> ControlStep<S::Command>`。
- `ControlStep<Command> = ControlFlow<(), (Command, bool)>`。
- `Continue((command, false))` 发送指令并继续；`Continue((command, true))` 发送最后一条有效指令后结束。
- `Break(())` 不携带算法指令，进入设备规定的结束协议；不等于急停，也不禁止设备协议所需的结束报文。不能捏造零指令填充退出。
- 进入结束路径后不能再调用回调。闭包只属于本次调用，无统一 `Send + 'static` 要求，不能在返回后保存。
- 原有 `control_with` 默认把 `(Command, bool)` 回调包装为 `Continue`，已有控制器闭包仍可使用。
- `hold_command(obs)` 是连续性 fallback，不是安全系统。
- `control_with_flow_async` 接收 async 回调并默认逐周期 `block_on`；`control_with_async` 再把普通 tuple 包装为 `Continue`。两者返回 `RobotResult<()>`，会话整体仍阻塞，不保证 Tokio reactor 或同线程兄弟 Future 的推进。

`Control` 是 blanket 用户入口：

```rust
robot.control_with::<TorqueControl<7>, _>(controller)?;
robot.control_with_flow::<TorqueControl<7>, _>(|_obs, _dt| {
    std::ops::ControlFlow::Break(())
})?;
```

这里的 Break 仅展示退出决策，不是真机启动或安全操作配方。原生异步会话另由 `AsyncControlWith` / `AsyncControlRhythm` 提供，详见 [原生异步控制](native-async-control.md)。状态有效性模型仍需按设备单独设计。

## ControlObservation

`ControlObservation` 是机器人对象上的 hook 注册能力：

- `before(|&Robot::State, Duration| ...) -> &mut Self`
- `after(|&Robot::State, Duration| ...) -> &mut Self`

observer 看完整 `Robot::State`，不是某个 `ControlSpace::Obs`。驱动负责存储和调用 observer。

## Controller 与轨迹工具

`roplat_control::*` 导出闭包生成函数，不导出 controller 对象体系。

常用函数：

- PD/PID：`joint_pd_control`、`joint_pd_tracking_control`、`joint_traj_pd_control`、`joint_pid_control`、`joint_pid_tracking_control`、`joint_traj_pid_control`
- base：`base_velocity_pid_control`、`base_velocity_pid_tracking_control`
- impedance：`joint_impedance_control`、`joint_traj_impedance_control`、`joint_impedance_tracking_control`、`joint_impedance_session`
- Cartesian impedance：`cartesian_impedance_control`、`cartesian_traj_impedance_control`、`cartesian_impedance_tracking_control`、`cartesian_impedance_session`
- dynamics：`gravity_compensation_control`、`computed_torque_control`、`computed_torque_tracking_control`

`roplat_trajectory::path_generate` 生成 controller-compatible target source：

- `constant_joint_target_fn`
- `joint_traj_target_fn`
- `joint_path_target_fn`
- `constant_pose_target_fn`
- `pose_traj_target_fn`
- `pose_path_target_fn`

`roplat_trajectory` 提供 COPP 规划和位置控制闭包：

- `plan_waypoints_traj_via_copp`
- `plan_path_traj_via_copp`
- `joint_traj_position_control`
- `copp_waypoints_joint_position_control`
- `copp_path_joint_position_control`

## 模型映射

`SpaceMap<From, To>`：

```rust
trait SpaceMap<From, To> {
    type Input;
    type Output;
    fn map(&self, input: Self::Input) -> RobotResult<Self::Output>;
}
```

`TypedSpaceMap<From, To>` 仅在 input/output 与 `MotionSpace` target 完全对齐时作为便利适配。

模型 trait：

- `ForwardKinematics<N>`：`JointSpace<N> -> FlangeSpace`
- `InverseKinematics<N>`：`FlangeSpace -> JointSpace<N>`
- `JacobianModel<N>`：`JointSpace<N> -> JacobianSpace<N>`
- `DynamicsModel<N>`：质量矩阵、Coriolis/centrifugal、gravity 映射
- `roplat_kinematics::ArmForwardKinematics<N>`：基于 DH 的 FK helper
- `roplat_kinematics::ArmInverseKinematics<N>`：解析/迭代 IK helper；用 `roplat_kinematics::dh_kinematics!(MyArm, N)`（或加 `inverse`）显式接入模型映射，旧 blanket impl 已移除

没有可靠模型数据时不要实现假模型。

## 原生异步控制

`AsyncControlWith<S>::control_native_async(&mut callback)` 返回完整会话的 Send Future。`AsyncControlCallback` 可借用自身状态跨 await，普通 FnMut→Send Future 有便利实现。设备启动、周期 I/O 和会话结束在同一异步路径内推进；基本行为能力不依赖 roplat feature。具体 runtime 由驱动说明；Franka 需要 Tokio I/O 与 timer。详见 [使用与实现边界](native-async-control.md)。

## Roplat 适配

`roplat_behavior::roplat::*` 由默认关闭的 `roplat` feature 启用，设备 trait 与工具无需该 feature。

- `MotionNode<R, S>`：`Resources = (R,)`，`Input = S::Target`，`Output = RobotResult<()>`。
- `SpaceMapNode<M, From, To>`：包装 `SpaceMap`。
- `SafetyNode<F, Command>`：转换 `(Command, bool)`，类型名不代表已实现硬件安全机制。
- `AsyncControlRhythm<R, S>`：通过原生异步会话运行，业务端口和归还规则与 ControlRhythm 相同。
- `ControlRhythm<R, S>`：`Resources = (R,)`，`Input = ()`，`Output = ()`，`Yield = (S::Obs, Duration)`，业务 `Feed = (S::Command, bool)`。
- `RobotException` 可转为 `RoplatError`（I/O 错误来源），图中可写 `motion?;`。

框架层的 `Execution<T>` 与业务 Feed 分开，分别表达完成、停止、失败。ControlRhythm 通过 `control_with_flow_async` 把已完成的 Feed 转为 `Continue`，把域停止/错误转为不带算法指令的 `Break`。先完成设备结束协议再返回，并保留域错误与设备收尾错误，避免成功周期因为错误聚合而增加分配。

所有协作退出归还不透明节点元组 `N`；不能丢弃在途 Future 来实现快速失败。机器人经 `&mut robot >> control.resources.0` 在 drive 期间借用，失败无需通过 Output 或回收钩子交还。设备 session 收尾与最终 Lifecycle 钩子仍是不同操作。

Node / Rhythm / 资源的创建层负责启用与关闭；在 System 内创建的机器人需实现 `roplat::Lifecycle`（连接/断开），作为参数传入则由调用方管理。传入子域不改变责任，多次 drive 不默认刷新对象状态。panic、丢弃 Future 和强制 abort 不属于协作归还保证。

应用以 `#[roplat::system]` 组织节点和多层节律，配合同仓 [roplat-development](../../roplat-development/SKILL.md)；本技能只补充设备能力知识，不重定义 DSL，也不指导用手写整条 process 链绕过核心语义。默认构建、roplat feature、外语绑定、native SDK、GUI、真机和性能应分开验证。

## 示例清单

`roplat_behavior/EXAMPLES.md` 是驱动能力示例目录标准。重点分类：

- 00 生命周期
- 01 状态读取
- 02 关节运动
- 03 笛卡尔运动
- 04 实时控制
- 05 reusable controllers
- 06 observation/logging
- 07 kinematics/dynamics model
- 08 end effector / IO
- 09 async / active backend
- 10 simulator-specific behavior
- 11 safety envelope
- 12 unsupported capability map

一个驱动不需要实现所有示例，但支持的能力应该有短例程证明。

## 仿真能力与模型路径

`PhysicsEngine` 负责推进世界，`SimulationBackend` 声明加载器 ID 与支持格式；`RobotDescription::SIMULATION_MODELS` 提供 URDF / MJCF / SDF 声明。`select_simulation_model` 优先后端专用模型，再选通用格式，最后仅对接受 URDF 的后端使用旧 `URDF` 回退。选择只校验声明交集，不转换文件或证明动力学等价；相对模型路径由后端的搜索规则解析。已选模型加载失败应报错，不能静默换模型。

开启 `roplat` feature 后，`roplat_behavior::roplat::SimulationRhythm::logical(engine).with_max_steps(n)` 拥有引擎，每个成功的 `step()` 后才向域发出 `()`；Input / Yield / Feed / Output / Resources 均为 `()`。`new(engine, interval)` 控制墙钟节奏，物理步长独立由 `engine.set_step_time(dt)` 设置。步数预算每次 drive 重置，世界状态不重置；引擎失败时该步不调用下游域。节律不自动 reset 或调用消耗引擎的 shutdown，后端用 RAII 清理，或在图外通过 `into_engine()` 显式收尾。具体后端的命令/状态刷新顺序及与真机的等价性仍须单独验证；本技能有限例程没有运行物理后端。
