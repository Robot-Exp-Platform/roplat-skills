//! Pick cycle with a mirrored follower. The leader runs two rhythm layers: the
//! task rhythm hands out one joint goal at a time, and the arm's own control
//! rhythm tracks a fresh smooth reference to that goal. The follower runs one
//! control rhythm for the whole task as a sibling domain; the two arms meet only
//! through latest-value sidebands, never through a graph edge or shared clock.

use std::time::Duration;

use robot_behavior::JointPositionControl;
use robot_behavior::roplat::AsyncControlRhythm;
use roplat::RoplatResult;
use roplat::comm::Latest;

use crate::adapters::{Handoff, TaskSequence};
use crate::nodes::{AnnounceFinished, JointServo, MinJerk, Mirror, ShareCommand};
use crate::sim::{DOF, Joints, SimArm};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

const SEGMENT: Duration = Duration::from_millis(300);

#[roplat::system]
pub async fn pick_and_follow(
    leader: SimArm,
    follower: SimArm,
    waypoints: Vec<Joints>,
    offset: Joints,
) -> RoplatResult<(SimArm, SimArm)> {
    let leader_command = Latest::<Joints>::new(1);
    let leader_finished = Latest::<bool>::new(1);

    let mut leader_arm = Handoff::new(leader);
    let mut tasks = TaskSequence::new(waypoints);
    let mut leader_control = ArmControl::new();
    let mut leader_servo = JointServo::default();
    let mut share = ShareCommand::new(leader_command.publisher());
    let mut announce = AnnounceFinished::new(leader_finished.publisher());

    let mut follower_arm = Handoff::new(follower);
    let mut follower_control = ArmControl::new();
    let mut mirror = Mirror::new(leader_command.subscriber(0), leader_finished.subscriber(0), offset);
    let mut follower_servo = JointServo::default();

    leader_arm >> tasks >> |(goal, robot)| {
        // Segment state: every goal gets its own reference from where the arm is.
        let mut path = MinJerk::towards(goal, SEGMENT);
        robot >> leader_control >> |(state, dt)| {
            state >> path.input.0;
            dt >> path.input.1;
            state >> leader_servo.input.0;
            path >> leader_servo.input.1;
            leader_servo >> share
        }
    };
    tasks.output >> announce;

    follower_arm >> follower_control >> |(state, _dt)| {
        state >> mirror;
        state >> follower_servo.input.0;
        mirror >> follower_servo.input.1;
        follower_servo
    };

    Ok((announce.output, follower_control.output))
}
