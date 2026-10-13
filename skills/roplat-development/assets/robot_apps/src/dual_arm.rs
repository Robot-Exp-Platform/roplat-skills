//! Two devices, two control rhythms, one System. Each arm keeps its own pace and
//! its algorithm inside its own domain; they meet only through a latest-value
//! sideband, never through a shared clock or a graph edge.

use robot_behavior::JointPositionControl;
use robot_behavior::roplat::AsyncControlRhythm;
use roplat::RoplatResult;
use roplat::comm::Latest;

use crate::SEGMENT;
use crate::nodes::{JointServo, MinJerk, Mirror, Reference, Share};
use crate::sim::{DOF, Joints, SimArm};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

#[roplat::system]
pub async fn leader_follower(
    mut leader: SimArm,
    mut follower: SimArm,
    goal: Joints,
    offset: Joints,
) -> RoplatResult<(SimArm, SimArm)> {
    let leader_command = Latest::<Reference>::new(1);

    let mut leader_control = ArmControl::new();
    let mut path = MinJerk::towards(goal, SEGMENT);
    let mut leader_servo = JointServo::default();
    let mut share = Share::new(leader_command.publisher());

    let mut follower_control = ArmControl::new();
    let mut mirror = Mirror::new(leader_command.subscriber(0), offset);
    let mut follower_servo = JointServo::default();

    &mut leader >> leader_control.resources.0;
    leader_control
        >> |(state, dt)| {
            state >> path.input.0;
            dt >> path.input.1;
            state >> leader_servo.input.0;
            path >> leader_servo.input.1;
            leader_servo >> share
        };

    &mut follower >> follower_control.resources.0;
    follower_control
        >> |(state, _dt)| {
            state >> mirror;
            state >> follower_servo.input.0;
            mirror >> follower_servo.input.1;
            follower_servo
        };

    Ok((leader, follower))
}
