# 使用已有驱动（use-robot-driver）

Use an existing `robot_behavior` driver without leaking driver implementation concerns into application code.

Read the shared [API and semantics reference](robot-behavior-overview.md) first. Then inspect the target driver's examples and trait impls before writing code. For Roplat applications, also use the sibling [roplat-development](../../roplat-development/SKILL.md) skill; this device capability guide does not define the graph DSL.

Read [native async control](native-async-control.md) when choosing or implementing a complete asynchronous session, composing concurrent control rhythms, or diagnosing a nested Tokio runtime. Preserve the existing blocking APIs.

## Role Boundary

Use this skill for:

- writing examples for `franka-rust`, `libjaka-rs`, `libhans-rs`, `libaubo-rs`, `rsbullet`, or adapters
- moving a robot in joint, flange, TCP, base, whole-body, gait, foot, or hand spaces
- running bounded realtime control sessions
- using reusable controller closures
- reading state, logging control observations, or querying model maps
- organizing driver examples according to `robot_behavior/EXAMPLES.md`

Do not use this guide to implement driver trait impls. Switch to [implement-robot-driver](implement-robot-driver.md).

## Import Rule

Prefer:

```rust
use robot_behavior::behavior::*;
```

Only import concrete driver types for connection and vendor-native APIs. Avoid `robot_behavior::driver::*` in application examples; it is for implementors and can create method ambiguity.

## Lifecycle Pattern

Use `Robot` methods directly:

```rust
robot.init()?;
robot.enable()?;
let state = robot.read_state()?;
robot.disable()?;
robot.shutdown()?;
```

Call only lifecycle methods that the driver documents as meaningful. Do not rely on `Drop` for normal shutdown.

For real robots:

- start with conservative driver-native speed/scale settings when available
- make examples terminate on their own
- explain physical movement in the example comments
- keep emergency stop and recovery paths visible to the operator

## Motion Pattern

Use typed spaces to state command meaning:

```rust
robot.move_to::<JointSpace<7>>(target_joint)?;
robot.move_to::<FlangeSpace>(target_pose)?;
robot.move_to::<TcpSpace>(target_pose)?;
```

Current semantics:

- `move_to` is blocking.
- `move_to_async` returns an inert Future; it runs only when polled or awaited.
- `move_to_sync` is deprecated. Do not add new uses.
- `move_traj`, `move_path`, and `move_waypoints` follow the driver's semantics. Verify whether the backend blocks or queues.
- `MotionFile` loads JSON trajectories through `move_traj_from_file` and `move_waypoints_from_file`.

Use these spaces first:

- joint arm motion: `JointSpace<N>`
- flange pose: `FlangeSpace`
- TCP pose: `TcpSpace`
- mobile/floating base: `BasePoseSpace`, `BaseVelocitySpace`
- whole-body robots: `WholeBodyJointSpace<N>`, `WholeBodyVelocitySpace<N>`, `WholeBodyTorqueSpace<N>`
- legged robots: `GaitSpace`, `FootSpace<LEG>`
- humanoids/hands: `HandSpace<HAND>`, `CenterOfMassSpace`

Use `Relative<S>` or `Inertial<S>` only if the driver implements that wrapped space.

## Trajectory Pattern

Dense trajectory:

```rust
robot.move_traj::<JointSpace<7>>(traj)?;
```

From file:

```rust
robot.move_traj_from_file::<JointSpace<7>>("examples/safe_joint_trajectory.json")?;
```

Waypoints:

```rust
robot.move_waypoints::<JointSpace<7>>(waypoints)?;
```

If the driver reports `UnprocessableInstructionError`, treat the capability as unsupported instead of silently switching to another motion mode.

For COPP-backed position commands, use:

```rust
let controller =
    robot_behavior::utils::trajectory::copp_waypoints_joint_position_control::<R, 7>(
        &waypoints,
        0.2,
    )?;
robot.control_with::<JointPositionControl<7>, _>(controller)?;
```

## Realtime Control Pattern

`Control::control_with::<S, _>` remains available for a controller returning `(Command, bool)`. The driver wraps each result in `ControlFlow::Continue`. Use `control_with_flow` when a cycle may finish without computing an algorithm command:

```rust
use std::ops::ControlFlow;
use robot_behavior::{Control, RobotResult, TorqueControl};

fn finish_session<R>(robot: &mut R) -> RobotResult<()>
where
    R: robot_behavior::ControlWith<TorqueControl<7>>,
{
    robot.control_with_flow::<TorqueControl<7>, _>(|_obs, _dt| {
        ControlFlow::Break(())
    })
}
```

This is a signature/exit example, not a hardware bring-up recipe. The canonical callback shape is:

```rust
FnMut(S::Obs, Duration) -> ControlStep<S::Command>
// ControlStep<Command> = ControlFlow<(), (Command, bool)>
```

Rules:

- `Continue((command, false))` sends the valid command and continues.
- `Continue((command, true))` sends a valid final command before normal completion.
- `Break(())` carries no algorithm command; the driver performs its own session termination protocol. It is not an emergency stop or a promise of no further protocol packets.
- Never invent a zero command to fill a failed or stopped cycle.
- Keep examples bounded by duration, sample count, or convergence.
- Do not call blocking I/O or re-enter the same robot from the control callback.
- `control_with_flow_async` and `control_with_async` accept async per-cycle callbacks, but return `RobotResult<()>` and block for the whole session. The generic default provides no Tokio reactor; individual drivers may create an internal runtime. Neither form guarantees progress of sibling futures on the calling thread.
- Verify each driver's actual execution model before using runtime-dependent futures. Use `AsyncControlWith` / `AsyncControlRhythm` for supported native asynchronous sessions; state-validity redesign remains separate.

Channel selection:

| Need | Control space |
|---|---|
| joint position servo | `JointPositionControl<N>` |
| joint velocity servo | `JointVelocityControl<N>` |
| joint torque/effort | `TorqueControl<N>` |
| torque with full arm context | `ArmTorqueControl<N>` |
| Cartesian pose command | `CartesianPoseControl<N>` |
| Cartesian velocity command | `CartesianVelocityControl<N>` |
| mobile base velocity | `BaseVelocityControl` |

## Controller Closures

Import:

```rust
use robot_behavior::controller::*;
```

Use controller helpers when you need closed-loop control, not normal point-to-point motion.

Examples:

```rust
let controller = joint_traj_pd_control(traj, [80.0; 7], [12.0; 7]);
robot.control_with::<TorqueControl<7>, _>(controller)?;
```

```rust
let controller = joint_impedance_tracking_control(target_fn, stiffness, damping);
robot.control_with::<TorqueControl<7>, _>(controller)?;
```

For Cartesian impedance use `ArmTorqueControl<N>` and a model implementing `ForwardKinematics<N> + JacobianModel<N>`.

Session handles:

- `joint_impedance_session` returns `(controller, handle)`
- `cartesian_impedance_session` returns `(controller, handle)`
- use `handle.finish()` to end the controller

## State And Observation

Use normalized arm state when available:

```rust
let arm_state = robot.state()?;
let q = arm_state.joint.meas.q;
let flange = arm_state.flange.meas.pose;
```

Use `ControlObservation` for full native state hooks:

```rust
robot
    .before(|state, dt| {
        let _ = (state, dt);
    })
    .after(|state, dt| {
        let _ = (state, dt);
    })
    .control_with::<JointPositionControl<7>, _>(controller)?;
```

Observers receive `&Robot::State`, not `S::Obs`. Keep them lightweight.

## Model Queries

When the driver has model support:

```rust
let pose = model.forward_kinematics(q)?;
let jacobian = model.jacobian(q)?;
let gravity = model.gravity(q, [0.0, 0.0, -9.81])?;
```

Use trait bounds for model examples:

```rust
R: ForwardKinematics<7> + JacobianModel<7> + DynamicsModel<7>
```

Do not require model traits in an application if the task can be written with live driver state.

## Roplat Usage

Enable `robot_behavior`'s `roplat` feature explicitly; it is off by default. Organize applications with `#[roplat::system]` and child rhythm domains, following [roplat-development](../../roplat-development/SKILL.md). Do not replace an application graph with a hand-written chain of `.process()` calls to bypass macro or ownership errors.

The robot is a **resource** (robot_behavior 0.7 / roplat 0.4): the graph lends it with `&mut robot` and gets it back from the user's Output.

```rust
&mut robot >> control >> |(state, dt)| { /* algorithm nodes */ servo };
control >> &mut robot;               // the drive Output returns the robot
&mut robot >> motion.input.0;
goal >> motion.input.1;
motion.0 >> &mut robot;              // returned whatever the motion result
motion.1?;                           // checked after the robot is home
```

Generic adapters:

- `MotionNode<R, S>` consumes `(R, S::Target)` and returns `(R, RobotResult<()>)`; the robot comes back on success and failure.
- `SpaceMapNode<M, From, To>` wraps a model map.
- `SafetyNode<F, Command>` transforms `(Command, bool)`; its name alone does not establish a hardware safety mechanism.
- `ControlRhythm<R, S>` (blocking session) and `AsyncControlRhythm<R, S>` (native async session): `Input = R`, `Yield = (S::Obs, Duration)`, business `Feed = (S::Command, bool)`, `Output = R`. When a drive does not complete they keep the robot and return it through `Rhythm::reclaim_input`; the generated graph puts it back in its slot.
- `RobotException` converts into `RoplatError` (I/O error source), so `motion.1?;` raises a device failure.

The framework result is separate: `Execution<R>` represents completed output, stopped execution or failure. Completed domain feed maps to `Continue((command, done))`; stopped/failed domains map to `Break(())`. Preserve a domain error and a device-session termination error if both occur.

Every cooperative exit returns the opaque node tuple `N`, which must be recovered by awaiting the in-flight domain. A lent robot comes back through Output or `reclaim_input`. The macro rejects a `?` that could leave the graph after the lend and skip the return (e.g. the return waits for `motion.1? >> verdict`), so return first and check with the standalone `motion.1?;`. To continue after a failed session, isolate the segment in a `roplat::rhythm::once()` child domain (created once in the head; it runs one cycle per drive) and handle its `.outcome`. Device session termination is separate from final resource shutdown. A stop that arrives while the lent robot waits on a compiler-inserted graph channel can drop it; that path is invalid and the robot is recreated, with any pre-loss handling done by the driver's `Drop`. Lending it again fails that domain with `RoplatError::Resource` (isolate with a `once()` child domain's `.outcome`).

Node/Rhythm/robot lifecycle belongs to the creating layer. A robot created with `let mut robot = ...` inside the System is enabled and closed by that System, so its type needs a `roplat::Lifecycle` impl (connect/disconnect); a robot passed in as a parameter is managed by the caller. A child domain uses external objects without reinitializing or closing them; repeated drives preserve state. `Robot` lifecycle methods are not automatically equivalent to `Lifecycle` hooks. Panic, dropping the outer Future and forced abort are outside cooperative-return guarantees.

Do not add background threads or mpsc hops in application code unless the backend requires them. A blocking `ControlRhythm` adapter is not evidence of true asynchronous transport or same-thread sibling progress.

## Simulators

For `rsbullet`, remember that some APIs intentionally mean "enqueue" or "register", not "block until physical motion ends". Respect the simulator's native step loop.

Do not assume simulator control semantics are identical to real robots just because they share typed spaces.

## Example Checklist

Follow `robot_behavior/EXAMPLES.md`:

- name examples like `02_00_move_joint_default.rs`, `04_02_control_torque.rs`, etc.
- keep each example short and focused
- include expected result and real-robot caution in moving examples
- include unsupported capability docs when a common trait is intentionally absent

## Validation

For a driver example set:

```powershell
cargo fmt
cargo check -p <crate> --all-targets --offline
# When graph integration is changed, also check the explicit roplat feature.
cargo package -p <crate> --allow-dirty --no-verify --list --offline
```

Use finite mock or pure-data tests first. Separate default-feature checks, roplat-feature checks, foreign-language builds, GUI, native SDK linkage and real-device validation. Check build-script asset writes before escalating a build. Do not run real-robot examples unless the user explicitly asks and the environment is safe; compilation alone is not hardware or realtime validation.
