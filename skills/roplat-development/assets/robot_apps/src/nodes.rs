//! Algorithm nodes. Their numerics are placeholders; the System files carry the
//! architecture. Each node owns one concern and is reusable across devices.

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
        Self {
            goal,
            duration,
            start: None,
            elapsed: Duration::ZERO,
        }
    }
}

impl Lifecycle for MinJerk {
    type Error = RoplatError;
}

impl Node for MinJerk {
    type Resources = ();
    type Input = (JointState<DOF>, Duration);
    type Output = Reference;

    async fn process(&mut self, (state, dt): Self::Input, _: ()) -> Reference {
        let start = *self.start.get_or_insert_with(|| measured(&state));
        self.elapsed += dt;
        let s = (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()).min(1.0);
        let blend = s.powi(3) * (10.0 - 15.0 * s + 6.0 * s * s);
        let target = std::array::from_fn(|i| start[i] + (self.goal[i] - start[i]) * blend);
        (target, s >= 1.0)
    }
}

/// Rate-limited joint position servo. It reports `done` once its reference has
/// finished and the measured position has reached it.
pub struct JointServo {
    max_step: f64,
    tolerance: f64,
}

impl Default for JointServo {
    fn default() -> Self {
        Self {
            max_step: 0.05,
            tolerance: 1e-3,
        }
    }
}

impl Lifecycle for JointServo {
    type Error = RoplatError;
}

impl Node for JointServo {
    type Resources = ();
    type Input = (JointState<DOF>, Reference);
    type Output = (Joints, bool);

    async fn process(&mut self, (state, (target, finished)): Self::Input, _: ()) -> Self::Output {
        let q = measured(&state);
        let command =
            std::array::from_fn(|i| q[i] + (target[i] - q[i]).clamp(-self.max_step, self.max_step));
        let reached = q
            .iter()
            .zip(target)
            .all(|(q, t)| (t - q).abs() < self.tolerance);
        (command, finished && reached)
    }
}

/// Publishes a device's latest command for other control domains; passes it on.
pub struct Share {
    latest: Publisher<Reference>,
}

impl Share {
    pub fn new(latest: Publisher<Reference>) -> Self {
        Self { latest }
    }
}

impl Lifecycle for Share {
    type Error = RoplatError;
}

impl Node for Share {
    type Resources = ();
    type Input = Reference;
    type Output = Reference;

    async fn process(&mut self, command: Reference, _: ()) -> Reference {
        self.latest.publish(command);
        command
    }
}

/// Follows another device's latest shared command at a fixed joint offset.
/// Until anything is shared, it holds the follower's own position.
pub struct Mirror {
    leader: Subscriber<Reference>,
    offset: Joints,
}

impl Mirror {
    pub fn new(leader: Subscriber<Reference>, offset: Joints) -> Self {
        Self { leader, offset }
    }
}

impl Lifecycle for Mirror {
    type Error = RoplatError;
}

impl Node for Mirror {
    type Resources = ();
    type Input = JointState<DOF>;
    type Output = Reference;

    async fn process(&mut self, state: JointState<DOF>, _: ()) -> Reference {
        match self.leader.latest() {
            Some(&(q, leader_done)) => {
                (std::array::from_fn(|i| q[i] + self.offset[i]), leader_done)
            }
            None => (measured(&state), false),
        }
    }
}
