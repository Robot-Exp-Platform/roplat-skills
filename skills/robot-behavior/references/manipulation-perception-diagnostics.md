# 抓取与取放、感知进场景、运行诊断、新设备驱动

本文介绍 2026-10-30 加入各仓 main 的几项能力。它们在冻结集 `2026.10-systest.1` 之后加入，**不属于任何冻结集**，也没有发布到 crates.io。使用前确认项目依赖的 roplat_behavior 源码已包含对应提交：

- roplat_behavior#41：抓取、语义与姿态约束；
- roplat_behavior#42：诊断；
- roplat_behavior#43：深度图与点云；
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
