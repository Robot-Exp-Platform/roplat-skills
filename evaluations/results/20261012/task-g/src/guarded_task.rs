//! Guarded goal sequence on an arm this System owns. The task rhythm hands out
//! one goal at a time; the arm's own control rhythm drives it through a
//! reference, a servo and a step guard. The System connects the arm before the
//! first goal and disconnects it on every way out, including a guard trip.

use robot_behavior::JointPositionControl;
use robot_behavior::roplat::AsyncControlRhythm;
use roplat::RoplatResult;
use roplat::comm::Publisher;
use roplat::rhythm::IntoRhythm;

use crate::nodes::{JointServo, MinJerk, Progress, StepGuard};
use crate::sim::{DOF, Joints, SimArm};
use crate::{HOME, MAX_STEP, SEGMENT};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

#[roplat::system]
pub async fn run_goals(
    goals: Vec<Joints>,
    link: Publisher<bool>,
    reached: Publisher<usize>,
) -> RoplatResult<Joints> {
    let mut arm = SimArm::new(HOME).with_link_status(link);
    let mut control = ArmControl::new();
    let mut servo = JointServo::default();
    let mut guard = StepGuard::new(MAX_STEP);

    let mut tasks = goals.into_iter().enumerate().into_rhythm();
    let mut progress = Progress::new(reached);

    tasks >> |(index, goal)| {
        let mut path = MinJerk::towards(goal, SEGMENT);
        // Bind the whole Yield: destructuring it as `(state, dt)` and fanning
        // `state` out to three nodes makes the current macro run `path` three
        // times per cycle.
        &mut arm >> control >> |sample| {
            sample >> path;
            sample.0 >> servo.input.0;
            path >> servo.input.1;
            sample.0 >> guard.input.0;
            servo >> guard.input.1;
            // A rejected command fails this cycle: the driver gets Break, not a substitute.
            guard?
        };
        // A failed segment aborts the whole task. Later goals were planned on the
        // assumption that this one was reached, and a rejected command means the
        // reference chain left its validated envelope; the operator decides what
        // happens next, not this program.
        control >> &mut arm;
        control >> progress.trigger;
        index >> progress;
    };

    Ok(arm.joints())
}
