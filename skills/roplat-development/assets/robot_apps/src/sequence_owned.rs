//! The same two layers, but the application creates the arm itself. Creation
//! place is lifecycle: this System connects the arm before the first task and
//! disconnects it after the last, even if a task fails.

use robot_behavior::JointPositionControl;
use robot_behavior::roplat::AsyncControlRhythm;
use roplat::RoplatResult;
use roplat::rhythm::IntoRhythm;

use crate::nodes::{JointServo, MinJerk};
use crate::sim::{DOF, Joints, SimArm};
use crate::{HOME, SEGMENT};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

#[roplat::system]
pub async fn visit(waypoints: Vec<Joints>) -> RoplatResult<(Joints, bool)> {
    let mut arm = SimArm::new(HOME);
    let mut tasks = waypoints.into_iter().into_rhythm();
    let mut control = ArmControl::new();
    let mut servo = JointServo::default();

    tasks
        >> |goal| {
            let mut path = MinJerk::towards(goal, SEGMENT);
            &mut arm >> control.resources.0;
            control
                >> |(state, dt)| {
                    state >> path.input.0;
                    dt >> path.input.1;
                    state >> servo.input.0;
                    path >> servo.input.1;
                    servo
                };
        };

    // Still connected here; the System disconnects it on the way out.
    Ok((arm.joints(), arm.is_connected()))
}
