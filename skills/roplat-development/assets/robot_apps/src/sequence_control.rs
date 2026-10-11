//! Two rhythm layers: the task rhythm hands out one goal at a time, and the
//! robot's own control rhythm drives that goal to completion. The robot is a
//! resource: each task lends it to the control session and gets it back.

use robot_behavior::JointPositionControl;
use robot_behavior::roplat::AsyncControlRhythm;
use roplat::RoplatResult;
use roplat::rhythm::IntoRhythm;

use crate::SEGMENT;
use crate::nodes::{JointServo, MinJerk};
use crate::sim::{DOF, Joints, SimArm};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

#[roplat::system]
pub async fn visit(arm: SimArm, waypoints: Vec<Joints>) -> RoplatResult<SimArm> {
    let mut tasks = waypoints.into_iter().into_rhythm();
    let mut control = ArmControl::new();
    let mut servo = JointServo::default();

    tasks >> |goal| {
        // Segment state: a fresh reference for every goal.
        let mut path = MinJerk::towards(goal, SEGMENT);
        &mut arm >> control >> |(state, dt)| {
            state >> path.input.0;
            dt >> path.input.1;
            state >> servo.input.0;
            path >> servo.input.1;
            servo
        };
        control >> &mut arm;
    };

    Ok(arm.into_inner())
}
