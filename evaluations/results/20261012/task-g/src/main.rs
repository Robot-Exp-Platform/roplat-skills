//! Runs a guarded goal sequence twice on a simulated arm: once within the step
//! limit, once with a goal whose reference is too fast for it.

mod guarded_task;
mod nodes;
mod sim;

use std::time::Duration;

use roplat::comm::{Latest, Subscriber};
use roplat::{RoplatError, RoplatResult};
use sim::Joints;

const HOME: Joints = [0.0, -0.8, 0.0, -2.2, 0.0, 1.6];
const SEGMENT: Duration = Duration::from_millis(200);
/// Largest joint motion one 2 ms command may ask for.
const MAX_STEP: f64 = 0.02;

struct Observers {
    link: Subscriber<bool>,
    reached: Subscriber<usize>,
}

async fn run(goals: Vec<Joints>) -> (RoplatResult<Joints>, Observers) {
    let link = Latest::new(1);
    let reached = Latest::new(1);
    let result = guarded_task::run_goals(goals, link.publisher(), reached.publisher()).await;
    (result, Observers { link: link.subscriber(0), reached: reached.subscriber(0) })
}

fn assert_near(actual: Joints, expected: Joints) {
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 2e-3, "{actual:?} != {expected:?}");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let above = [0.3, -0.5, 0.1, -2.0, 0.0, 1.5];
    let grasp = [0.3, -0.3, 0.1, -1.9, 0.0, 1.4];
    // A 2 rad swing of the base joint in one 200 ms segment: its reference
    // needs several times the per-cycle step limit.
    let swing = [2.3, -0.3, 0.1, -1.9, 0.0, 1.4];

    let (result, mut observed) = run(vec![above, grasp, above, HOME]).await;
    let joints = result.expect("every segment stays within the step limit");
    assert_near(joints, HOME);
    assert_eq!(observed.reached.latest(), Some(&3), "all four goals reached");
    assert_eq!(observed.link.latest(), Some(&false), "the System disconnected the arm");

    let (result, mut observed) = run(vec![above, grasp, swing, HOME]).await;
    match result {
        Err(RoplatError::NodeProcessing(message)) => {
            assert!(message.starts_with("step guard: joint 0"), "{message}")
        }
        other => panic!("expected the step guard to abort the task, got {other:?}"),
    }
    assert_eq!(
        observed.reached.latest(),
        Some(&1),
        "goals 0 and 1 reached; goal 2 rejected, goal 3 never started"
    );
    assert_eq!(observed.link.latest(), Some(&false), "disconnected on the failure path too");

    println!("task-g: guarded sequence completed; swing goal aborted the task; arm disconnected both times");
}
