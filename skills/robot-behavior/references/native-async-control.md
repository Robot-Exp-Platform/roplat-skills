# Native asynchronous control

The internal baseline adds `AsyncControlWith<S>` alongside the existing `ControlWith<S>`. Check the pinned source revision; published package version numbers alone do not identify this API.

| Need | API | Execution contract |
|---|---|---|
| Synchronous motion | `move_to` | Blocks until completion/failure; ordinary driver path |
| Existing async cycle callback | `control_with_async` / `control_with_flow_async` | Blocking session; per-cycle callback may await |
| Session polled by caller | `AsyncControl::control_native_async::<S, _>(&mut callback).await` | Full session Future, including device I/O and termination |
| Roplat graph with native async driver | `robot_behavior::roplat::AsyncControlRhythm<R, S>` | Awaits native session; keeps the same Yield/Feed/Input/Output and cooperative N-return contract |

Franka implements six native control channels using its existing Tokio UDP backend and asynchronous TCP session boundaries. Its native path requires an outer Tokio runtime with I/O and time enabled. Its old blocking async wrapper still creates a local runtime; do not place that wrapper directly inside a Tokio task. Synchronous move_to remains synchronous.

The callback trait is `AsyncControlCallback<Obs, Command>: Send`. Its `call(&mut self, obs, duration)` returns `impl Future<Output = ControlStep<Command>> + Send`. Ordinary `FnMut -> Send Future` callbacks have a blanket implementation. For a controller that borrows its mutable state across await, implement the callback trait on a named state struct. The driver borrows that callback for the session and awaits each invocation fully. Do not require per-cycle boxing, spawning, cloning nodes, or a `'static` callback merely to bridge this interface.

The robot still paces the cycle: receive state, execute one complete domain, send a valid command. Async waiting can let sibling futures progress; it does not promise fairness during continuous CPU work or a hardware deadline. A parent graph's dependent successor still waits until the complete drive returns. Compose application graphs with the core `#[system]` DSL.

Completed Feed is committed; a concurrent stop is observed at the next callback. A domain without a command returns Stopped/Err, producing Break and device-specific termination. Await the in-flight domain to regain N. Normal completion, cooperative stop and error preserve N. The lent robot returns as Output on completion and through `reclaim_input` otherwise; dropped futures and panic do not promise asynchronous cleanup. Franka prevents reuse after an unconfirmed session rather than pretending the device stopped.

Feature boundaries: basic drivers and native async behavior need no roplat feature. Enable `robot_behavior/roplat` for graph adapters and `rsbullet/roplat` for SimRhythm. Preserve independent manifests, use the matching fixed Git revisions, and apply local source patches at the final workspace root. Cargo workspace inheritance is only for members of that workspace, not for independent driver repositories.

Validate with finite mock and loopback tests: genuinely Pending I/O/callbacks, same-task sibling progress, nested System domains, errors/stops/N return, and sequential synchronous/asynchronous sessions. Separate CPU batch means from per-cycle latency percentiles and from hardware measurements.
