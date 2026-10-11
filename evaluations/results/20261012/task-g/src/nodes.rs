//! Algorithm nodes. Each node owns one concern; the System file carries the
//! architecture.

use std::time::Duration;

use robot_behavior::JointState;
use roplat::comm::Publisher;
use roplat::{Lifecycle, Node, RoplatError, RoplatResult};

use crate::sim::{DOF, Joints};

/// A joint target and whether its producer has finished.
pub type Reference = (Joints, bool);

fn measured(state: &JointState<DOF>) -> Joints {
    state.meas.q.unwrap_or([0.0; DOF])
}

/// Minimum-jerk reference from the first observed position to `goal`.
pub struct MinJerk {
    goal: Joints,
    duration: Duration,
    start: Option<Joints>,
    elapsed: Duration,
}

impl MinJerk {
    pub fn towards(goal: Joints, duration: Duration) -> Self {
        Self { goal, duration, start: None, elapsed: Duration::ZERO }
    }
}

impl Lifecycle for MinJerk {
    type Error = RoplatError;
}

impl Node for MinJerk {
    type Input = (JointState<DOF>, Duration);
    type Output = Reference;

    async fn process(&mut self, (state, dt): Self::Input) -> Reference {
        let start = *self.start.get_or_insert_with(|| measured(&state));
        self.elapsed += dt;
        let s = (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()).min(1.0);
        let blend = s.powi(3) * (10.0 - 15.0 * s + 6.0 * s * s);
        let target = std::array::from_fn(|i| start[i] + (self.goal[i] - start[i]) * blend);
        (target, s >= 1.0)
    }
}

/// Proportional joint position servo around the reference. It does not limit
/// its own steps; that is the guard's job. Reports `done` once the reference
/// has finished and the measured position has reached it.
pub struct JointServo {
    gain: f64,
    tolerance: f64,
}

impl Default for JointServo {
    fn default() -> Self {
        Self { gain: 0.5, tolerance: 1e-3 }
    }
}

impl Lifecycle for JointServo {
    type Error = RoplatError;
}

impl Node for JointServo {
    type Input = (JointState<DOF>, Reference);
    type Output = (Joints, bool);

    async fn process(&mut self, (state, (target, finished)): Self::Input) -> Self::Output {
        let q = measured(&state);
        let command = std::array::from_fn(|i| target[i] + self.gain * (target[i] - q[i]));
        let reached = q.iter().zip(target).all(|(q, t)| (t - q).abs() < self.tolerance);
        (command, finished && reached)
    }
}

/// Rejects a command that would move any joint further than `max_step` from
/// its measured position in one cycle. It never repairs or replaces the
/// command: a rejection is an error, and the session ends without sending it.
pub struct StepGuard {
    max_step: f64,
}

impl StepGuard {
    pub fn new(max_step: f64) -> Self {
        Self { max_step }
    }
}

impl Lifecycle for StepGuard {
    type Error = RoplatError;
}

impl Node for StepGuard {
    type Input = (JointState<DOF>, (Joints, bool));
    type Output = RoplatResult<(Joints, bool)>;

    async fn process(&mut self, (state, (command, done)): Self::Input) -> Self::Output {
        let q = measured(&state);
        for (joint, (q, c)) in q.iter().zip(command).enumerate() {
            let step = (c - q).abs();
            if step > self.max_step {
                return Err(RoplatError::NodeProcessing(format!(
                    "step guard: joint {joint} command moves {step:.4} rad in one cycle, limit {:.4} rad",
                    self.max_step
                )));
            }
        }
        Ok((command, done))
    }
}

/// Publishes the index of the most recently reached goal for an operator display.
pub struct Progress {
    reached: Publisher<usize>,
}

impl Progress {
    pub fn new(reached: Publisher<usize>) -> Self {
        Self { reached }
    }
}

impl Lifecycle for Progress {
    type Error = RoplatError;
}

impl Node for Progress {
    type Input = usize;
    type Output = ();

    async fn process(&mut self, goal: usize) {
        self.reached.publish(goal);
    }
}
