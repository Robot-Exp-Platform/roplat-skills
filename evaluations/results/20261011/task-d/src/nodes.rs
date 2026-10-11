//! Algorithm and sideband nodes. Each node owns one concern; none knows which
//! rhythm drives it.

use std::marker::PhantomData;
use std::time::Duration;

use robot_behavior::JointState;
use roplat::comm::{Publisher, Subscriber};
use roplat::{Lifecycle, Node, RoplatError};

use crate::sim::{DOF, Joints};

/// A joint target and whether the producer of that target has finished.
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

/// Rate-limited joint position servo. Reports `done` once its reference has
/// finished and the measured position has reached it.
pub struct JointServo {
    max_step: f64,
    tolerance: f64,
}

impl Default for JointServo {
    fn default() -> Self {
        Self { max_step: 0.05, tolerance: 1e-3 }
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
        let command = std::array::from_fn(|i| {
            q[i] + (target[i] - q[i]).clamp(-self.max_step, self.max_step)
        });
        let reached = q.iter().zip(target).all(|(q, t)| (t - q).abs() < self.tolerance);
        (command, finished && reached)
    }
}

/// Publishes the leader's latest joint command for other control domains and
/// passes the cycle's Feed through unchanged.
pub struct ShareCommand {
    latest: Publisher<Joints>,
}

impl ShareCommand {
    pub fn new(latest: Publisher<Joints>) -> Self {
        Self { latest }
    }
}

impl Lifecycle for ShareCommand {
    type Error = RoplatError;
}

impl Node for ShareCommand {
    type Input = (Joints, bool);
    type Output = (Joints, bool);

    async fn process(&mut self, (command, done): (Joints, bool)) -> (Joints, bool) {
        self.latest.publish(command);
        (command, done)
    }
}

/// Announces that a device's whole task has finished and passes the device on.
pub struct AnnounceFinished<R> {
    finished: Publisher<bool>,
    _robot: PhantomData<fn(R) -> R>,
}

impl<R> AnnounceFinished<R> {
    pub fn new(finished: Publisher<bool>) -> Self {
        Self { finished, _robot: PhantomData }
    }
}

impl<R> Lifecycle for AnnounceFinished<R> {
    type Error = RoplatError;
}

impl<R> Node for AnnounceFinished<R> {
    type Input = R;
    type Output = R;

    async fn process(&mut self, robot: R) -> R {
        self.finished.publish(true);
        robot
    }
}

/// Follows the leader's latest command at a fixed joint offset. The reference
/// is finished once the leader has announced the end of its task. Until the
/// leader commands anything, it holds the follower's own position.
pub struct Mirror {
    leader_command: Subscriber<Joints>,
    leader_finished: Subscriber<bool>,
    offset: Joints,
}

impl Mirror {
    pub fn new(
        leader_command: Subscriber<Joints>,
        leader_finished: Subscriber<bool>,
        offset: Joints,
    ) -> Self {
        Self { leader_command, leader_finished, offset }
    }
}

impl Lifecycle for Mirror {
    type Error = RoplatError;
}

impl Node for Mirror {
    type Input = JointState<DOF>;
    type Output = Reference;

    async fn process(&mut self, state: JointState<DOF>) -> Reference {
        // Read the finish flag first: a command read afterwards is then the
        // leader's final one.
        let finished = self.leader_finished.latest().copied().unwrap_or(false);
        match self.leader_command.latest() {
            Some(q) => (std::array::from_fn(|i| q[i] + self.offset[i]), finished),
            None => (measured(&state), finished),
        }
    }
}
