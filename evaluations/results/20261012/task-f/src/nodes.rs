//! Algorithm nodes for the tracking cell. Each node owns one concern.

use robot_behavior::JointState;
use roplat::comm::{Publisher, Subscriber};
use roplat::{Lifecycle, Node, RoplatError};

use crate::conveyor::PartReport;
use crate::sim::{DOF, Joints};

/// A joint target and whether the producer of that target has finished.
pub type Reference = (Joints, bool);

/// The tracking joint: base yaw points the arm at the part.
pub const YAW: usize = 0;

/// Base yaw that points at a part `position` along a belt `offset` in front.
pub fn aim(offset: f64, position: f64) -> f64 {
    position.atan2(offset)
}

fn measured(state: &JointState<DOF>) -> Joints {
    state.meas.q.unwrap_or([0.0; DOF])
}

/// Publishes the belt's latest part report to whoever watches the belt.
pub struct PartBroadcast {
    latest: Publisher<PartReport>,
}

impl PartBroadcast {
    pub fn new(latest: Publisher<PartReport>) -> Self {
        Self { latest }
    }
}

impl Lifecycle for PartBroadcast {
    type Error = RoplatError;
}

impl Node for PartBroadcast {
    type Input = PartReport;
    type Output = ();

    async fn process(&mut self, report: PartReport) {
        self.latest.publish(report);
    }
}

/// Turns the latest part report into a joint reference: yaw toward the part,
/// other joints held where they are. Finished once the part has left.
/// Until the belt reports anything, it holds the arm's own position.
pub struct PartTracker {
    belt: Subscriber<PartReport>,
    offset: f64,
}

impl PartTracker {
    pub fn new(belt: Subscriber<PartReport>, offset: f64) -> Self {
        Self { belt, offset }
    }
}

impl Lifecycle for PartTracker {
    type Error = RoplatError;
}

impl Node for PartTracker {
    type Input = JointState<DOF>;
    type Output = Reference;

    async fn process(&mut self, state: JointState<DOF>) -> Reference {
        let mut target = measured(&state);
        match self.belt.latest() {
            Some(report) => {
                target[YAW] = aim(self.offset, report.position);
                (target, report.left)
            }
            None => (target, false),
        }
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
