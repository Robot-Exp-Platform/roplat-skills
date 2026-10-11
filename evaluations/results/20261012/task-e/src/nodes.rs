//! Algorithm nodes for the inspection cell. Each node owns one concern.

use robot_behavior::JointState;
use roplat::{Lifecycle, Node, RoplatError};

use crate::sim::{DOF, Joints};

/// A joint target and whether the producer of that target has finished.
pub type Reference = (Joints, bool);

fn measured(state: &JointState<DOF>) -> Joints {
    state.meas.q.unwrap_or([0.0; DOF])
}

/// Takes one sample per control cycle at the pose where the arm arrived and
/// asks to hold that pose until enough samples are in.
pub struct Measure {
    samples: usize,
    cycles: usize,
    hold: Option<Joints>,
}

impl Measure {
    pub fn samples(samples: usize) -> Self {
        Self { samples, cycles: 0, hold: None }
    }

    /// Control cycles this measurement has used so far.
    pub fn cycles(&self) -> usize {
        self.cycles
    }
}

impl Lifecycle for Measure {
    type Error = RoplatError;
}

impl Node for Measure {
    type Input = JointState<DOF>;
    type Output = Reference;

    async fn process(&mut self, state: JointState<DOF>) -> Reference {
        let hold = *self.hold.get_or_insert_with(|| measured(&state));
        self.cycles += 1;
        (hold, self.cycles >= self.samples)
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
