//! Robot-shaped roplat applications against a simulated arm. The three
//! `sequence_*` files solve the same waypoint task with different structures.

mod dual_arm;
mod nodes;
mod sequence_control;
mod sequence_motion;
mod sequence_owned;
mod sim;

use std::time::Duration;

use roplat::{Lifecycle, RoplatResult};
use sim::{Joints, SimArm};

const HOME: Joints = [0.0, -0.8, 0.0, -2.2, 0.0, 1.6];
const SEGMENT: Duration = Duration::from_millis(200);

fn assert_near(actual: Joints, expected: Joints) {
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 2e-3, "{actual:?} != {expected:?}");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> RoplatResult<()> {
    let above = [0.3, -0.5, 0.1, -2.0, 0.0, 1.5];
    let grasp = [0.3, -0.3, 0.1, -1.9, 0.0, 1.4];
    let waypoints = vec![above, grasp, above, HOME];

    // This caller creates the arm, so it connects and disconnects it; the
    // Systems below only lend it to their users.
    let mut arm = SimArm::new(HOME);
    arm.on_init().await?;

    let arm = sequence_control::visit(arm, waypoints.clone()).await?;
    assert_near(arm.joints(), HOME);

    let arm = sequence_motion::visit(arm, waypoints.clone()).await?;
    assert_near(arm.joints(), HOME);
    assert!(
        arm.is_connected(),
        "a System leaves an arm it did not create connected"
    );

    let (joints, connected) = sequence_owned::visit(waypoints).await?;
    assert_near(joints, HOME);
    assert!(
        connected,
        "the creating System connects the arm before its tasks"
    );

    let offset = [0.0, 0.0, 0.0, 0.0, 0.0, 0.2];
    let mut follower = SimArm::new(std::array::from_fn(|i| HOME[i] + offset[i]));
    follower.on_init().await?;
    let (mut leader, mut follower) =
        dual_arm::leader_follower(arm, follower, grasp, offset).await?;
    assert_near(leader.joints(), grasp);
    assert_near(
        follower.joints(),
        std::array::from_fn(|i| grasp[i] + offset[i]),
    );
    leader.on_shutdown().await?;
    follower.on_shutdown().await?;

    println!("robot_apps: three waypoint structures reached HOME; follower mirrored leader");
    Ok(())
}
