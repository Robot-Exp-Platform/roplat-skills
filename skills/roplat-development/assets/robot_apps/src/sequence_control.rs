//! Two rhythm layers: the task rhythm hands out one goal at a time, and the
//! robot's own control rhythm drives that goal to completion. The robot is a
//! resource: each task borrows it for the control session without moving it
//! into business Input or Output.

use roplat::RoplatResult;
use roplat::rhythm::IntoRhythm;
use roplat_behavior::JointPositionControl;
use roplat_behavior::roplat::AsyncControlRhythm;

use crate::SEGMENT;
use crate::nodes::{JointServo, MinJerk};
use crate::sim::{DOF, Joints, SimArm};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

#[roplat::system]
pub async fn visit(mut arm: SimArm, waypoints: Vec<Joints>) -> RoplatResult<SimArm> {
    let mut tasks = waypoints.into_iter().into_rhythm();
    let mut control = ArmControl::new();
    let mut servo = JointServo::default();

    tasks
        >> |goal| {
            // Segment state: a fresh reference for every goal.
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

    Ok(arm)
}
