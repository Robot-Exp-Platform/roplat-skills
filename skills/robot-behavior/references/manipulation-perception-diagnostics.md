# 抓取与取放、感知进场景、标定与轨迹跟随、运行诊断、新设备驱动

本文介绍 2026-10-30 加入各仓 main 的几项能力。它们在冻结集 `2026.10-systest.1` 之后加入，**不属于任何冻结集**，也没有发布到 crates.io。使用前确认项目依赖的 roplat_behavior 源码已包含对应提交：

- roplat_behavior#41：抓取、语义与姿态约束；
- roplat_behavior#42：诊断；
- roplat_behavior#43：深度图与点云；
- roplat_behavior#44：笛卡尔轨迹跟随；
- roplat_behavior#45：手眼标定；
- roplat_behavior#46：凹形物体；
- roplat_behavior#48：抓取成功判据；
- roplat_behavior#49：路径简化与平滑；
- rsbullet#21：仿真中的笛卡尔位姿控制；
- 各驱动仓的 main。

按 [version.json](version.json) 的约定保留源码身份。

## 选哪个包

| 要做的事 | 用 | 对应 ROS 的 |
|---|---|---|
| 命名位姿（home、ready）、末端工具中心点与工具连杆 | `roplat_scene::{NamedPoses, EndEffector}` | SRDF |
| 抓取候选（长方体、圆柱、球） | `roplat_grasp::grasps` | MoveIt Grasps |
| 预抓取、接近、抓取、抬起的整段规划 | `roplat_grasp::plan_pick` | MoveIt pick |
| 规划时保持姿态（端杯竖直） | `roplat_trajectory::{AxisConstraint, constrained}` | 路径约束 |
| 仿真 RGB-D 相机（Bullet 软件渲染，无显示也可用） | `rsbullet::{CameraSpec, RsBullet::render_rgbd, RsBullet::camera}`（实现 `Camera` 契约） | Gazebo 相机插件 |
| 深度图 → 点 → 场景障碍 | `roplat_scene::points::depth_to_points`、`PlanningScene::add_points` | Octomap 进规划场景 |
| 料箱、货架等凹形物体进场景 | `PlanningScene::add_concave_object`、`add_trimesh` | MoveIt 的网格碰撞物体 |
| 相机安装位置标定 | `roplat_frames::calibration::calibrate_hand_eye` | easy_handeye、MoveIt Calibration |
| 按时间播放工具位姿轨迹 | `roplat_control::{CartesianTrajectoryFollower, cartesian_line}` | 笛卡尔轨迹控制器 |
| 判断是否抓住 | `roplat_grasp::{GraspCheck, carried_mass}` | 各夹爪驱动各自的判断 |
| 规划后删去多余路点、圆滑拐角 | `roplat_trajectory::{simplify_path, smooth_path}` | OMPL 的路径简化器 |
| 频率、延迟、健康状态 | `roplat_diagnostics` | topic hz/delay、diagnostic_updater/aggregator |
| Robotiq 2F 夹爪 / ATI 力传感器 / WitMotion IMU | `librobotiq` / `libatift` / `libwitmotion` | 厂商驱动包 |

## 取放

工具坐标系约定：`z` 为接近方向（指向物体），`y` 为两指闭合方向。`EndEffector` 用同一约定，把工具中心点放在臂的某个连杆上。

```rust
use roplat_grasp::{GraspOptions, PickOptions, grasps, plan_pick};
use roplat_scene::EndEffector;

let hand = EndEffector::new("hand", "panda_link8", tcp_on_link8).with_links(["panda_hand", "panda_leftfinger", "panda_rightfinger"]);
let candidates = grasps(&block_geometry, &block_pose, &GraspOptions { max_opening: 0.08, ..Default::default() });
let plan = plan_pick(&scene, &hand, "block", Some("table"), &candidates, start, lower, upper,
                     |link_pose, seed| ik_for_link(link_pose, seed), PickOptions::default())?;
// plan.to_pregrasp → 用 COPP 定时并执行；plan.approach → 执行后用 Gripper 闭合；plan.lift → 执行。
```

- 候选按优劣排序，从上方接近的排在前面。从下方接近的默认不给出：`GraspOptions::from_below`。
- `plan_pick` 只做几何规划，不闭合手指，也不给轨迹定时。闭合用 `Gripper::move_gripper`，定时用 `roplat_trajectory` 的 COPP。
- 失败时 `PickError::attempts` 给出每个候选的失败原因：不可达、碰撞、无路径，或接近、抬起走到一半。按原因调整，不要只加大重试次数。
- 场景本身不被修改：抬起阶段在副本上附着物体。执行后，在自己的场景里调用 `scene.attach(object, hand.link(), &hand.links(), &plan.grasp_configuration())`。

## 感知进场景

```rust
use roplat_scene::points::depth_to_points;

let points = depth_to_points(&depth_m, &camera_info, 0.1..=2.0, 2);  // 光学系：z 前、x 右、y 下
scene.add_points("seen", &points, 0.02, camera_pose_in_world)?;      // 2 cm 体素
```

- 深度图须已校正，函数不处理畸变。
- 仿真中，用 `sim.render_rgbd(&spec)` 得到公制深度；`spec.info()` 给出与渲染一致的内参，`spec.pose` 是光学系在世界中的位姿，可直接作为 `add_points` 的 `pose`。没有命中的像素为无穷大，函数会自动跳过。
- 新读数要替换旧读数时，先 `remove_object("seen")` 再重新添加。
- 点没有专门的消息类型：图像与点云的流类型由数据流任务定义，这里只用 `[f64; 3]` 切片。

- 料箱、货架这类凹形物体用 `add_concave_object`（网格文件，需要面信息：OBJ 的 `f` 行或 STL）或 `add_trimesh`（代码中给出的三角形）加入。`add_object` 会取凸包，把内部空间填满，手伸不进去。分解使用 V-HACD：发布构建约 0.2 s，未优化时慢约 200 倍。调试构建请在工作区 `Cargo.toml` 中加入 `[profile.dev.package.parry3d-f64] opt-level = 3`。

## 相机标定与工具位姿轨迹

```rust
use roplat_frames::calibration::{HandEye, HandEyeSample, calibrate_hand_eye};

// 每个工位：法兰在基座系中的位姿（正运动学），以及同一时刻标定板在相机光学系中的位姿（检测器给出）
let calib = calibrate_hand_eye(&samples, HandEye::EyeInHand)?;
assert!(calib.translation_rms < 0.002);                        // 先看离散程度，再信结果
let tree = FrameTree::builder().fixed("panda_link8", "camera", calib.camera_pose()) /* … */;
```

- 各工位要绕至少两根明显不同的轴转动，每次转动超过几度，否则报 `Degenerate`。标定板检测（ChArUco、AprilTag）不在 roplat 中，用相机厂商 SDK 或 OpenCV 得到位姿后传入。
- 眼在手外（相机固定）时用 `HandEye::EyeToHand`，结果是相机在基座系中的位姿。

```rust
use roplat_control::{CartesianTrajectoryFollower, cartesian_line};

let waypoints = cartesian_line(&from, &to, Duration::from_secs(2), Robot::CONTROL_PERIOD);
let mut follower = CartesianTrajectoryFollower::<_, 7>::cycle_timed(waypoints)?.with_path_tolerance(0.01, 0.05);
robot.control_with_flow::<CartesianPoseControl, _>(|state, dt| match follower.step_by(dt, &state) {
    Ok(step) => ControlFlow::Continue(step),
    Err(_) => ControlFlow::Break(()),
})?;
```

- 路点的坐标系与驱动的 `CartesianPoseControl` 一致，一般是法兰在基座系中的位姿；容差与测得的法兰位姿比较。
- 仿真中，rsbullet 的 `RsBulletDevice` 实现 `CartesianPoseControl`：每周期用 Bullet 逆解把法兰位姿（末端连杆在世界系中的位姿）转为位置电机目标，可以先在仿真里验证笛卡尔控制器。
- 在多个线程里同时建 rsbullet 仿真世界并加载带 OBJ 网格的模型（如 Panda）会崩溃，因为 Bullet 的 OBJ 缓存是进程级且无锁的。在库层修复之前，请依次加载。
- 超出容差时返回错误，不修改命令；要停止就 `Break`。

## 路径简化与平滑

```rust
use roplat_trajectory::{plan_joint_path, simplify_path, smooth_path};

let path = plan_joint_path(start, goal, lower, upper, |q| scene.is_free(q), options)?;
let path = simplify_path(&path, options.resolution, |q| scene.is_free(q));   // 删去直线可跳过的路点
let path = smooth_path(&path, 3, options.resolution, |q| scene.is_free(q));  // 圆滑拐角，2–4 轮通常够用
// 再交给 plan_waypoints_traj_via_copp 定时
```

- 两者都保持端点不变，并检查每一段新路径。拐角处的切割弦无效时，依次改用八分之一、十六分之一的切割，仍无效则保留拐角，所以贴近障碍物的路径不会被拉穿。
- 每轮平滑后路点约翻倍。

## 判断是否抓住

```rust
use roplat_grasp::{GraspCheck, GraspOutcome, carried_mass};

let check = GraspCheck::for_grasp(&plan.grasp, 0.003);
let state = gripper.read_gripper()?;
match check.judge(state.width, Some(state.grasped != 0)) {
    GraspOutcome::Held { .. } => { /* 抬起 */ }
    outcome => { /* 空抓、宽度不符或与驱动判断不一致：松开，换下一个候选 */ }
}
// 抬起前后在同一腕部姿态各读一次力，沿重力方向的增量即负载质量
let mass = carried_mass(before, after, gravity_in_sensor_frame);
```

- 判断依据是任何夹爪都会报告的开口宽度；驱动自己的 `grasped` 作为第二意见，两者不一致时为 `Disputed`。
- `carried_mass` 假定传感器报告工具对传感器的力；若传感器报告的是反作用力，把两次读数对调。

## 运行诊断

```rust
use roplat_diagnostics::{Aggregator, FrequencyCheck, ProbeNode};

let (probe, handle) = ProbeNode::<State, _>::new(clock.clone(),
    FrequencyCheck::new("arm/state", 1000.0..=1000.0, 0.1, Duration::from_millis(5)));
// 图中：state >> probe; probe >> 下游;  图外：handle.lock().status(&clock.now())
```

- 节点类需要开启 `roplat` feature。
- 阈值规则：没有事件为 ERROR；频率超出期望范围加上容差为 WARN；静默超过超时为 STALE。
- 状态中报告 p99 与最大周期。均值正常时，偶发的超时也能从这两项看出。
- 时间一律取自 `Clock`，仿真与回放时间同样适用。

## 新设备驱动

三个驱动都在 drives 工作区中，各自是独立仓库，都**还没有上真机**。各仓 README 写有到货后的验证步骤。

| 驱动 | 契约 | 连接 | 测试替身 |
|---|---|---|---|
| `librobotiq`（2F-85、2F-140、Hand-E） | `Gripper` | Modbus RTU、RS-485，115200 8N1，`serial` feature | `MockRobotiq`（`mock` feature） |
| `libatift`（ATI Net F/T、Axia） | `ForceTorqueSensor` | UDP RDT，端口 49152 | `MockNetFt`（`mock` feature） |
| `libwitmotion`（WT901、HWT901B、JY901） | `ImuSensor` | 串口，`serial` feature | 由协议生成的字节流 |

- 三个驱动的测试都运行 `roplat_behavior::conformance` 的契约检查。新驱动照此做：在 dev-dependencies 中开启 roplat_behavior 的 `conformance` feature，对 mock 运行 `check_gripper`、`check_force_torque_sensor` 或 `check_imu`。
- ATI 的计数换算（counts per force、counts per torque）因标定而异，从传感器网页上的 `netftapi2.xml` 读取后设置，不要直接用默认值 1e6。
