//! Inspection cell: the task rhythm hands out one pose at a time. At each pose
//! the driver first moves there point-to-point, then the arm's own control
//! rhythm holds it while a measurement samples once per device cycle. The arm
//! is a resource lent to the motion and then to the control session.

use robot_behavior::roplat::{AsyncControlRhythm, MotionNode};
use robot_behavior::{JointPositionControl, JointSpace};
use roplat::RoplatResult;

use crate::SAMPLES;
use crate::nodes::{JointServo, Measure};
use crate::sim::{DOF, Joints, SimArm};
use crate::tasks::TaskList;

type MoveJoints = MotionNode<SimArm, JointSpace<DOF>>;
type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

/// Visits every pose and returns the arm with the control cycles each
/// measurement used, in pose order.
#[roplat::system]
pub async fn inspect(arm: SimArm, poses: Vec<Joints>) -> RoplatResult<(SimArm, Vec<usize>)> {
    // Each pose cycle reports its measurement's cycle count; the drive collects them.
    let mut tasks = TaskList::new(poses);
    let mut motion = MoveJoints::new();
    let mut control = ArmControl::new();
    let mut servo = JointServo::default();

    tasks >> |pose| {
        // Per-pose state: every pose starts a fresh measurement.
        let mut measure = Measure::samples(SAMPLES);

        &mut arm >> motion.input.0;
        pose >> motion.input.1;
        motion.0 >> &mut arm;
        motion.1?;

        &mut arm >> control >> |(state, _dt)| {
            state >> measure;
            state >> servo.input.0;
            measure >> servo.input.1;
            servo
        };
        control >> &mut arm;

        measure.cycles()
    };

    Ok((arm.into_inner(), tasks.output))
}
