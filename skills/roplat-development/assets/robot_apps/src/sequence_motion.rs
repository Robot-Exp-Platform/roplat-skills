//! One rhythm layer: `MotionNode` replaces the control rhythm. The driver plans
//! and executes each point-to-point motion; the application only orders goals.

use robot_behavior::JointSpace;
use robot_behavior::roplat::MotionNode;
use roplat::RoplatResult;
use roplat::rhythm::IntoRhythm;

use crate::sim::{DOF, Joints, SimArm};

type MoveJoints = MotionNode<SimArm, JointSpace<DOF>>;

#[roplat::system]
pub async fn visit(arm: SimArm, waypoints: Vec<Joints>) -> RoplatResult<SimArm> {
    let mut tasks = waypoints.into_iter().into_rhythm();
    let mut motion = MoveJoints::new();

    tasks >> |goal| {
        &mut arm >> motion.input.0;
        goal >> motion.input.1;
        motion.0 >> &mut arm; // (robot, result): the robot comes back either way
        motion.1?; // a failed motion ends the task after the robot is home
    };

    Ok(arm.into_inner())
}
