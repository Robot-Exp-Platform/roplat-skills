# 抓取与取放、感知进场景、标定与轨迹跟随、运行诊断、新设备驱动

本文介绍 2026-10-30、10-31 加入各仓 main 的几项能力。它们在冻结集 `2026.10-systest.1` 之后加入，**不属于任何冻结集**，也没有发布到 crates.io。使用前确认项目依赖的 roplat_behavior 源码已包含对应提交：

- roplat_behavior#41：抓取、语义与姿态约束；
- roplat_behavior#42：诊断；
- roplat_behavior#43：深度图与点云；
- roplat_behavior#44：笛卡尔轨迹跟随；
- roplat_behavior#45：手眼标定；
- roplat_behavior#46：凹形物体；
- roplat_behavior#48：抓取成功判据；
- roplat_behavior#49：路径简化与平滑；
- rsbullet#21：仿真中的笛卡尔位姿控制；
- roplat_behavior#50：规划场景与坐标树时间缓冲的连接；
- roplat_behavior#51：笛卡尔轨迹跟随的节点形式；
- roplat_behavior#52：整段运动检查（2026-10-31，下同）；
- roplat_behavior#53：点云与桌面分割（新 crate `roplat_perception`）；
- roplat_behavior#54：机器人自身点滤波与凹形连杆；
- roplat_behavior#55：标定板检测与位姿；
- 各驱动仓的 main（含新仓 libspacemouse-rs）。

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
| 棋盘格角点与标定板位姿 | `roplat_frames::target::{detect_chessboard, target_pose}` | OpenCV `findChessboardCorners`、`solvePnP` |
| 整段检查一段关节直线运动（不按分辨率采样） | `PlanningScene::motion_is_free`、`roplat_trajectory::plan_joint_motion` | FCL 连续碰撞检测 |
| 桌面与桌上物体的分割和形状拟合 | `roplat_perception::segment_tabletop` | PCL 桌面分割、tabletop_object_detector |
| 已知模型的位姿精修 | `roplat_perception::icp` | PCL / Open3D ICP |
| 深度读数去掉机械臂自身 | `PlanningScene::without_robot_points` | MoveIt 的 self filter |
| 凹形连杆（张开的手掌、中空工具） | `PlanningScene::decompose_link` | 网格碰撞几何 |
| 按时间播放工具位姿轨迹 | `roplat_control::{CartesianTrajectoryFollower, cartesian_line}` | 笛卡尔轨迹控制器 |
| 判断是否抓住 | `roplat_grasp::{GraspCheck, carried_mass}` | 各夹爪驱动各自的判断 |
| 规划后删去多余路点、圆滑拐角 | `roplat_trajectory::{simplify_path, smooth_path}` | OMPL 的路径简化器 |
| 频率、延迟、健康状态 | `roplat_diagnostics` | topic hz/delay、diagnostic_updater/aggregator |
| Robotiq 2F 夹爪 / ATI 力传感器 / WitMotion IMU | `librobotiq` / `libatift` / `libwitmotion` | 厂商驱动包 |
| 3Dconnexion 3D 鼠标遥操作 | `libspacemouse` + `roplat_control::cartesian_teleop_control` | spacenav_node、teleop_twist |

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
- 相机装在臂上时，用图像时刻的相机位姿，而不是处理时的位姿：开启 `roplat_frames` 的 feature `scene` 后，`scene::add_sensed_points(&buffer, &mut scene, "seen", &points, 0.02, WORLD, CAMERA, image_stamp)` 从记录的关节状态插值出该时刻的相机位姿；被跟踪的物体用 `scene::place_objects` 移到其坐标系在某一时刻的位姿。
- 点没有专门的消息类型：图像与点云的流类型由数据流任务定义，这里只用 `[f64; 3]` 切片。
- 机械臂在画面里时，先去掉落在它身上的点，否则它会成为自己的障碍物：`let points = scene.without_robot_points(&points_in_world, &q_at_image, 0.02);`。`q_at_image` 是图像时刻的关节位置；margin 取 2–3 cm，盖住深度噪声与碰撞几何和真实表面之间的差。附着在连杆上的物体一并去掉。

- 料箱、货架这类凹形物体用 `add_concave_object`（网格文件，需要面信息：OBJ 的 `f` 行或 STL）或 `add_trimesh`（代码中给出的三角形）加入。`add_object` 会取凸包，把内部空间填满，手伸不进去。分解使用 V-HACD：发布构建约 0.2 s，未优化时慢约 200 倍。调试构建请在工作区 `Cargo.toml` 中加入 `[profile.dev.package.parry3d-f64] opt-level = 3`。

## 相机标定与工具位姿轨迹

```rust
use roplat_frames::calibration::{HandEye, HandEyeSample, calibrate_hand_eye};

// 每个工位：法兰在基座系中的位姿（正运动学），以及同一时刻标定板在相机光学系中的位姿（检测器给出）
let calib = calibrate_hand_eye(&samples, HandEye::EyeInHand)?;
assert!(calib.translation_rms < 0.002);                        // 先看离散程度，再信结果
let tree = FrameTree::builder().fixed("panda_link8", "camera", calib.camera_pose()) /* … */;
```

- 各工位要绕至少两根明显不同的轴转动，每次转动超过几度，否则报 `Degenerate`。
- 每个工位的标定板位姿可以直接从图像得到：

```rust
use roplat_frames::target::{Chessboard, DetectOptions, detect_chessboard, target_pose};

let board = Chessboard { columns: 9, rows: 6, square: 0.025 };   // 内角点数；columns + rows 取奇数
let corners = detect_chessboard(&gray, width, height, &board, &DetectOptions::default())
    .ok_or("board not found")?;                                    // 整块板都要在画面内
let target = target_pose(&corners, &board.corners(), &camera_info).ok_or("no pose")?;
assert!(target.rms < 0.5);                                         // 重投影误差（像素）
samples.push(HandEyeSample { flange: flange_pose, target: target.pose });
```

- 角点顺序在各视角下一致：第一个角点挨着深色的角上方格。这要求 `columns + rows` 为奇数（如 OpenCV 常用的 9×6）；为偶数时只能确定到相差半圈。
- 图像须已校正（不处理畸变）。方格在图像中小于约 10 像素时，调 `DetectOptions` 的 `radius` 与 `blur`。ChArUco、AprilTag（部分可见、带编号）仍需 OpenCV 等外部检测器。
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
- 开启 `roplat_control` 的 `roplat` feature 后，跟随器也是 roplat 节点：在 `ControlRhythm` 图中写 `sample >> follower; follower?`，拒绝即 `Break`，用 `CartesianTrackingError::find` 从 drive 的错误中取回原因，与关节轨迹跟随相同。

## 整段运动检查

按 `resolution` 采样检查一段运动时，两个采样点之间连杆扫过的区域没有被检查，薄板或棱角可能被跳过。整段检查用保守推进给出证明，不需要选分辨率：

```rust
use roplat_trajectory::{plan_joint_motion, simplify_motion, smooth_motion};

let path = plan_joint_motion(start, goal, lower, upper, |a, b| scene.motion_is_free(a, b), options)?;
let path = simplify_motion(&path, |a, b| scene.motion_is_free(a, b));
let path = smooth_motion(&path, 3, |a, b| scene.motion_is_free(a, b));
// 要知道第一次接触的位置：scene.motion_contact(&a, &b) 给出比例与接触对
```

- 一段运动在离障碍物 0.1 mm（`MOTION_TOLERANCE`）以内就算接触；`motion_contact` 报告的比例之前的位形都无碰撞。
- `motion_is_free` 先检查终点和每 0.05 rad 的采样，通过后才做证明，所以被挡住的运动很快被拒绝。Panda 上一段 0.2 rad 的自由运动约 0.3 ms，约为 0.02 rad 采样的 1.75 倍；整个规划反而略快，因为拒绝更快。
- `plan_cartesian_path` 与 `plan_pick` 仍按采样检查。

## 桌面物体分割

```rust
use roplat_perception::{TabletopOptions, segment_tabletop};

// 世界系中的点（先去掉机械臂自身的点）
let found = segment_tabletop(&points, &TabletopOptions::default())?;
for (k, object) in found.objects.iter().enumerate() {
    scene.add_object(&format!("object{k}"), &object.shape.geometry, object.shape.pose)?;
    let candidates = roplat_grasp::grasps(&object.shape.geometry, &object.shape.pose, &grasp_options);
}
```

- 流程：5 mm 体素降采样；RANSAC 找在 `up` 方向 0.2 rad 内水平的主平面作为桌面；取桌面轮廓上方 1–50 cm 的点；按 2 cm 间隙聚类；每个聚类拟合立在桌上的长方体（轮廓最小面积矩形）或直立圆柱（轮廓圆拟合），取轮廓吻合更好的一种。
- 点必须在已知 `up` 方向的坐标系中（世界或基座），不能是相机光学系。
- 相互接触的物体会合成一个聚类；形状只有长方体与圆柱两种，物体底面视为落在桌面上。
- 已知模型时，用 `icp` 精修位姿：以传感点为 source、完整模型为 target，从猜测位姿的逆出发，结果再取逆。方向反过来时，模型看不见的一面会被拉偏。
- 发布构建下，一张 320×240 深度图（约 7.7 万点）的分割约 30 ms。

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

这些驱动都在 drives 工作区中，各自是独立仓库，都**还没有上真机**。各仓 README 写有到货后的验证步骤。

| 驱动 | 契约 | 连接 | 测试替身 |
|---|---|---|---|
| `librobotiq`（2F-85、2F-140、Hand-E） | `Gripper` | Modbus RTU、RS-485，115200 8N1，`serial` feature | `MockRobotiq`（`mock` feature） |
| `libatift`（ATI Net F/T、Axia） | `ForceTorqueSensor` | UDP RDT，端口 49152 | `MockNetFt`（`mock` feature） |
| `libwitmotion`（WT901、HWT901B、JY901） | `ImuSensor` | 串口，`serial` feature | 由协议生成的字节流 |
| `libspacemouse`（SpaceMouse Compact、Wireless、Pro、Enterprise，SpaceNavigator 等） | 无契约：给出 `InputAxes`，接 `cartesian_teleop_control` | USB HID，`hid` feature（hidapi） | 录制的 HID 报告 |

- 三个驱动的测试都运行 `roplat_behavior::conformance` 的契约检查。新驱动照此做：在 dev-dependencies 中开启 roplat_behavior 的 `conformance` feature，对 mock 运行 `check_gripper`、`check_force_torque_sensor` 或 `check_imu`。
- ATI 的计数换算（counts per force、counts per torque）因标定而异，从传感器网页上的 `netftapi2.xml` 读取后设置，不要直接用默认值 1e6。
- SpaceMouse：`SpaceMouse::open(Settings::default())?` 后，把 `move || mouse.next_input()` 交给 `cartesian_teleop_control`，并设 `TeleopMapping::enable_button` 作为手持开关。坐标为 x 右、y 前（推离自己）、z 上；轴的正负方向尚未在实物上核对，先按 README 的步骤检查再接机器人。Linux 需要 hidraw 的 udev 规则。
