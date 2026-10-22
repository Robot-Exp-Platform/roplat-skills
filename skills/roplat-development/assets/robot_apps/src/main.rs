//! Robot-shaped roplat applications against a simulated arm. The three
//! `sequence_*` files solve the same waypoint task with different structures;
//! `plan_hard` keeps a heavy planner off the control loop, `task_events` serves
//! operator commands, and `dual_arm` couples two arms through a sideband.

mod dual_arm;
mod nodes;
mod plan_hard;
mod sequence_control;
mod sequence_motion;
mod sequence_owned;
mod sim;
mod task_events;

use std::time::Duration;

use roplat::{Lifecycle, RoplatResult};
use sim::{Joints, SimArm};
use task_events::Command;

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

    // The planner blocks its thread for 40 ms per solve; the 2 ms control loop
    // must not notice. Without `.hard()` the longest gap would be a whole solve.
    let solve_time = Duration::from_millis(40);
    let (arm, longest_gap) =
        plan_hard::plan_while_controlling(arm, vec![above, grasp, HOME], solve_time).await?;
    assert_near(arm.joints(), HOME);
    assert!(
        longest_gap < solve_time / 2,
        "a control cycle waited {longest_gap:?} for the planner"
    );

    // Operator commands: the first motion faults once and is retried, one
    // target is beyond the joint limits, one would hit the fixture.
    let mut arm = arm;
    arm.fail_next_moves(1);
    let beyond = [0.0, -0.5, 0.0, -3.5, 0.0, 1.5];
    let fixture = [1.5, -0.5, 0.0, -2.0, 0.0, 1.5];
    let script = vec![
        Command::Goto(above),
        Command::Goto(beyond),
        Command::Dwell(Duration::from_millis(5)),
        Command::Goto(fixture),
        Command::Goto(grasp),
        Command::Goto(HOME),
    ];
    let commands = script.len();
    let (arm, outcome) = task_events::run_console(arm, script, 1.2..1.8).await?;
    assert_near(arm.joints(), HOME);
    assert_eq!(outcome.attempts, 4, "above twice, grasp and HOME once");
    assert_eq!(outcome.out_of_reach, vec![beyond]);
    assert_eq!(outcome.blocked, vec![fixture]);
    assert!(
        (1..=commands).contains(&outcome.panel.len()),
        "refresh presses merge into at most one panel cycle each"
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

    println!(
        "robot_apps: three waypoint structures reached HOME; planning stayed off the control loop; \
         operator commands were checked and retried; follower mirrored leader"
    );
    Ok(())
}
