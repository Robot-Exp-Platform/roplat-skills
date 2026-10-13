//! One rhythm layer: `MotionNode` replaces the control rhythm. The driver plans
//! and executes each point-to-point motion; the application only orders goals.

use robot_behavior::JointSpace;
use robot_behavior::roplat::MotionNode;
use roplat::RoplatResult;
use roplat::rhythm::IntoRhythm;

use crate::sim::{DOF, Joints, SimArm};

type MoveJoints = MotionNode<SimArm, JointSpace<DOF>>;

#[roplat::system]
pub async fn visit(mut arm: SimArm, waypoints: Vec<Joints>) -> RoplatResult<SimArm> {
    let mut tasks = waypoints.into_iter().into_rhythm();
    let mut motion = MoveJoints::new();

    tasks
        >> |goal| {
            &mut arm >> motion.resources.0;
            goal >> motion;

            motion?; // business failure ends the task; the resource remains owned
        };

    Ok(arm)
}
