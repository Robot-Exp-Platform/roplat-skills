# 实现或审查驱动（implement-robot-driver）

Implement or review a driver so it honestly fits the current `roplat_behavior` abstraction.

Read the shared [API and semantics reference](robot-behavior-overview.md) first. Then read the target crate's current source before editing. For application graph examples, combine this guide with the sibling [roplat-development](../../roplat-development/SKILL.md) skill; keep this guide focused on device capabilities.

Read [native async control](native-async-control.md) when choosing or implementing a complete asynchronous session, composing concurrent control rhythms, or diagnosing a nested Tokio runtime. Preserve the existing blocking APIs.

## Role Boundary

Use this skill for:

- implementing a new hardware or simulator driver
- refactoring a downstream driver to current `roplat_behavior` traits
- reviewing whether a driver implements capabilities honestly
- deciding whether a vendor API should become a shared trait
- adding examples that prove implemented capabilities
- updating Python/C++ stubs after Rust semantics settle

Do not use this skill for ordinary application examples. Switch to `use-robot-driver`.

## Design Rule

`roplat_behavior` is a capability contract layer. A driver should expose what the backend can really do and explicitly reject what it cannot do.

Avoid:

- fake torque control
- presenting a blocking async-callback session as truly asynchronous transport
- fake dynamics or kinematics
- one-off shared traits for a single vendor function
- duplicated controller logic already present in `roplat_control`
- hidden extra sockets/sessions that compete for the same robot resource

## Import Rule

Inside driver crates:

```rust
use roplat_behavior::driver::*;
```

This imports user-facing behavior traits plus implementation traits: `MoveTo`, `MoveTraj`, and `ControlWith`.

`RobotResult` and `RobotException` are root exports, not included in these glob imports. Import the ones your implementation uses explicitly with `use roplat_behavior::{RobotException, RobotResult};`.

## Implementation Order

Implement in this order:

1. `Robot`
2. `Joints<N>` and/or `EndPoint`
3. native-to-normalized state conversion
4. shape capability: `Arm<N>`, `MobileBase`, `Quadruped<N>`, or `Humanoid<N>`
5. `MoveTo<S>` per supported motion space
6. `MoveTraj<S>` per supported trajectory space
7. `ControlWith<S>` per supported realtime channel
8. `ControlObservation`
9. model maps: FK, IK, Jacobian, dynamics
10. Roplat adapters only when requested
11. FFI/stubs only after Rust semantics are stable
12. examples and package checks

## Robot

Choose `Robot::State` as the full native state or a driver-owned full state. It should be rich enough for lifecycle reads and `ControlObservation`.

Implement:

```rust
impl Robot for MyRobot {
    type State = MyState;

    const CONTROL_PERIOD: f64 = 0.001;

    fn version() -> String { ... }
    fn read_state(&mut self) -> RobotResult<Self::State> { ... }
}
```

Override lifecycle methods only when the backend has real semantics:

- `init`
- `shutdown`
- `enable`
- `disable`
- `reset`
- `stop`
- `pause`
- `resume`
- `emergency_stop`
- `clear_emergency_stop`
- `is_moving`
- `waiting_for_finish`

Keep one exclusive backend owner for communication/control. `read_state`, motion, and realtime control must not silently create competing transport resources.

## Units And Limits

Convert vendor units at the boundary. Public `roplat_behavior` values use:

- rad, rad/s, rad/s^2, rad/s^3
- N*m, N*m/s
- meters, m/s, m/s^2, m/s^3
- spatial velocity `[vx, vy, vz, wx, wy, wz]`
- spatial wrench `[fx, fy, fz, tx, ty, tz]`

Implement `Joints<N>` for articulated robots:

```rust
impl Joints<7> for MyRobot {
    const JOINT_DEFAULT: [f64; 7] = [...];
    const JOINT_MIN: [f64; 7] = [...];
    const JOINT_MAX: [f64; 7] = [...];
    const JOINT_VEL_BOUND: [f64; 7] = [...];
    const JOINT_ACC_BOUND: [f64; 7] = [...];
    const JOINT_JERK_BOUND: [f64; 7] = [...];
    const TORQUE_BOUND: [f64; 7] = [...];
    const TORQUE_DOT_BOUND: [f64; 7] = [...];
}
```

Implement `EndPoint` only for meaningful Cartesian limits.

Do not leave real hardware limits as `f64::MAX` if the vendor manual provides real values.

## State Conversion

Map native telemetry into current state types:

- use `JointState<N>` for joint measured/commanded/desired samples
- use `SpatialSample` for pose/twist/wrench samples
- use `ArmState<N>` for serial arms
- use `BaseState`, `MobileBaseState`, `QuadrupedState<N>`, or `HumanoidState<N>` for other forms

Preserve `meas`, `cmd`, and `des` when the backend exposes them. For unavailable sample fields use `JointSample::EMPTY` / `SpatialSample::EMPTY`, or clear validity with setters such as `set_q(None)` / `set_pose(None)`; do not assign the old public Option fields or invent measured values.

## Arm And Other Capability Bundles

Implement `Arm<N>` only for serial arms. `Arm` is not the root abstraction for all robots.

`Arm<N>` requires:

- `Robot`
- `Joints<N>`
- `EndPoint`
- `MoveTo<JointSpace<N>>`
- `MoveTo<FlangeSpace>`

Also implement:

- `state(&mut self) -> RobotResult<ArmState<N>>`
- `set_load`
- joint/flange getters
- `with_joint_*`, `with_cartesian_*`, `with_rotation_*`

For mobile bases, quadrupeds, or humanoids, use the category traits and whole-body spaces instead of squeezing everything into `Arm`.

## MoveTo

Implement `MoveTo<S>` per supported motion space:

```rust
impl MoveTo<JointSpace<7>> for MyRobot {
    fn move_to(&mut self, target: [f64; 7]) -> RobotResult<()> {
        // blocking motion
        Ok(())
    }
}
```

Rules:

- `move_to` must block until motion completion or error.
- funnel semantic spaces when appropriate, e.g. `TcpSpace -> FlangeSpace -> JointSpace -> hardware`
- validate limits before sending commands when the backend does not
- return `RobotException::UnprocessableInstructionError` for unsupported semantic spaces
- override `move_to_async` only with a real async backend; the returned Future must be inert until polled

Do not implement async by spawning a thread around the blocking call unless the driver explicitly documents that as its backend model.

## MoveTraj

Implement `MoveTraj<S>` when the backend can consume sampled trajectories or when the driver deliberately funnels trajectories through a supported control channel.

Current trait requires:

- `move_traj`
- `move_path`
- `move_waypoints`

If `move_path` or `move_waypoints` is unsupported, implement it as:

```rust
Err(RobotException::UnprocessableInstructionError(
    "driver does not support continuous path planning; use move_traj".to_string(),
))
```

Do not leave a runtime panic. Do not silently reinterpret sparse waypoints as dense trajectories unless that is documented and safe.

Use `roplat_trajectory` only when the driver really wires in COPP or sampled path generation.

## ControlWith

Implement `ControlWith<S>::control_with_flow` one channel at a time. Signature excerpt:

```rust
fn control_with_flow<F>(&mut self, closure: F) -> RobotResult<()>
where
    F: FnMut(S::Obs, Duration) -> ControlStep<S::Command>;
// ControlStep<Command> = std::ops::ControlFlow<(), (Command, bool)>
```

Required semantics:

- The session blocks until it completes or fails; the callback is scoped to this call, with no general `Send + 'static` requirement.
- Acquire the device observation, call the controller, then handle its decision.
- `Continue((command, false))`: send the computed command and continue.
- `Continue((command, true))`: send that final valid command, then finish normally.
- `Break(())`: no algorithm command exists for this cycle; enter the device termination protocol without manufacturing one. Protocol-required termination/continuity packets remain device-specific.
- Do not invoke the callback again after either ending path.
- Preserve primary failure and device termination failure when both occur. Do not add allocations to each successful cycle solely for error aggregation.
- Never store the scoped closure after the session returns or add another loop competing for the same robot's transport ownership.

`control_with` remains a default convenience wrapper, mapping its `(Command, bool)` callback to `Continue`. Existing controller generators can use it unchanged. New drivers implement the flow primitive, not only the wrapper.

Implement `hold_command(obs)` from meaningful channel state when supported. It is a continuity fallback, not a safety system, and must not be presented as universally safe zero torque/velocity. Safety remains in the device's validated configuration, filters, limits, watchdogs and stop path.

`control_with_flow_async` accepts an async per-cycle callback but is itself blocking; by default it uses per-cycle `futures::executor::block_on`. `control_with_async` wraps its ordinary command tuple in `Continue`. Override the flow-async primitive only when the backend preserves this session contract with its own runtime. Do not introduce a true async-session or state-validity redesign as an incidental compatibility fix.

For regression tests, mock the transport and verify that `Break` does not produce an algorithm command, `Continue((cmd, true))` sends exactly one final command, the callback is not repeated after exit, and termination failures are retained. These tests do not replace hardware validation.

## ControlObservation

Implement when observers can be called around each control cycle:

```rust
impl ControlObservation for MyRobot {
    fn before<H>(&mut self, observer: H) -> &mut Self
    where
        H: FnMut(&Self::State, Duration) + Send + 'static,
    { ... }

    fn after<H>(&mut self, observer: H) -> &mut Self
    where
        H: FnMut(&Self::State, Duration) + Send + 'static,
    { ... }
}
```

Store `ControlObserver<Self::State>` on the robot. Observers receive full native state, not `S::Obs`.

Document whether observers run in a realtime context.

## Simulators

Do not force a multi-entity, step-driven simulator into standard `ControlWith` if the semantics are registered/queued.

Acceptable designs:

- normal `ControlWith` only for a blocking single-robot session
- simulator-native registration methods for queued controllers
- simulator-specific Roplat rhythm that owns the world step, distributes state, gathers commands, and applies them

Keep typed spaces and control channel markers where useful, but be honest about execution semantics.

## Model Maps

Use `SpaceMap<From, To>` for typed mappings:

```rust
impl SpaceMap<JointSpace<7>, FlangeSpace> for MyRobot {
    type Input = [f64; 7];
    type Output = Pose;

    fn map(&self, q: Self::Input) -> RobotResult<Self::Output> {
        Ok(Pose::default())
    }
}
```

Implement model helper traits only with reliable data:

- `roplat_kinematics::ArmForwardKinematics<N>` from DH data
- `roplat_kinematics::ArmInverseKinematics<N>` for IK; add `roplat_kinematics::dh_kinematics!(MyRobot, N)` (or `..., inverse`) to connect DH helpers to model maps
- `ForwardKinematics<N>`
- `InverseKinematics<N>`
- `JacobianModel<N>`
- `DynamicsModel<N>`

Do not fake model traits to satisfy examples.

## Vendor-Native APIs

Keep one-off vendor functions as inherent methods:

- collision behavior
- IO
- gripper
- firmware settings
- brake release
- payload identification
- recovery procedures

Promote to a shared trait only after multiple drivers need the same behavior and the semantics can be written cleanly.

## Roplat

The `roplat` feature is off by default. Keep device capabilities usable without it. The generic adapters use a separate resource channel:

- `MotionNode<R, S>`: `Resources = (R,)`, `Input = S::Target`, `Output = RobotResult<()>`.
- `SpaceMapNode<M, From, To>` and `SafetyNode<F, Command>` have no external resources (`Resources = ()`).
- `ControlRhythm<R, S>` / `AsyncControlRhythm<R, S>`: `Resources = (R,)`, `Input = ()`, `Output = ()`, `Yield = (S::Obs, Duration)`, business `Feed = (S::Command, bool)`.
- `RobotException` converts into `RoplatError`, allowing `motion?` in a graph.

`drive` returns `(Execution<()>, N)`. Every cooperative exit returns N after awaiting the in-flight domain. A stopped or failed domain becomes `ControlFlow::Break(())`, so device termination runs without an invented algorithm command. Preserve domain and termination errors if both occur.

Node and Rhythm implementers select a resource type tuple; core `ResourceSet` supplies mutable references whose lifetime covers only that process/drive call. Declare `(R,)` and receive `(robot,): (&'call mut R,)`; multiple resources use `(R, Camera)` and `(&'call mut R, &'call mut Camera)`. Business Input/Output stay separate. Do not add a recovery hook, a held `Option<R>`, or a resource return Output. Rust permits replacing the borrowed body; this contract does not enforce identity preservation. A rhythm that already owns a device internally can keep that design with `Resources = ()`. Device session cleanup and final object lifecycle remain distinct.

The creating scope owns Node/Rhythm/resource initialization and shutdown. Child domains only use passed objects, and repeated drives do not reset state. Keep normal-path overhead bounded and avoid allocation introduced solely to aggregate future errors.

Application examples should use `#[roplat::system]` with child domains, borrowing the robot with `&mut robot >> user.resources.0`; see the sibling [roplat-development](../../roplat-development/SKILL.md) skill. Writing driver internals is legitimate; rewriting the application as a manual `.process()` chain to bypass the graph is not an adaptation strategy.

Simulator-level rhythms and sample-validity redesign are separate changes requiring explicit design alignment.

## FFI And Stubs

Rust semantics are source of truth.

Only update `.pyi`, `.hpp`, `to_py`, `to_cxx`, or `to_c` after the Rust trait surface is stable. If the task is not about FFI, avoid touching these files.

## Examples

Use `roplat_behavior/EXAMPLES.md` as capability checklist. A driver should provide examples for every supported common behavior:

- lifecycle and state read
- joint motion
- Cartesian motion when supported
- realtime control channels
- reusable controllers
- observation
- model maps
- vendor-native end effector / IO
- async backend when real
- safety envelope
- unsupported capability map

Keep examples short. Prefer capability proof over demos with extra structure.

## Validation

From workspace root:

```powershell
cargo fmt
cargo check -p <crate> --all-targets --offline
cargo package -p <crate> --allow-dirty --no-verify --list --offline
```

For `roplat_behavior` itself:

```powershell
cd roplat_behavior
cargo check -p roplat_behavior --all-targets --offline
cargo check -p roplat_behavior --features roplat --all-targets --offline
cargo package -p roplat_behavior --allow-dirty --no-verify --list --offline
```

Use finite mock and pure-data tests before any device run. Do not run GUI demonstrations, ignored unbounded tests or SDK bring-up as routine verification. Check build-script asset writes separately. Use real-robot runs only when explicitly requested and physically safe; report exactly which features/platforms were tested.

## Review Checklist

- [ ] no fake async
- [ ] no fake torque/control/model trait
- [ ] `move_to` blocks
- [ ] `control_with_flow` blocks and does not store scoped closures
- [ ] `Break` carries no algorithm command; final `Continue((cmd, true))` semantics and termination errors are tested
- [ ] async callback methods are documented as blocking sessions
- [ ] default build works without roplat; graph feature and cooperative return are checked separately
- [ ] unsupported paths return `UnprocessableInstructionError`
- [ ] state preserves `meas` / `cmd` / `des`
- [ ] units are converted at boundary
- [ ] examples cover supported traits
- [ ] package list includes required stubs/assets and excludes junk
