//! Inspection cell on a simulated arm: visit each pose, hold it for a short
//! measurement, and report how many control cycles each measurement used.

mod inspection;
mod nodes;
mod sim;
mod tasks;

use roplat::{Lifecycle, RoplatResult};
use sim::{Joints, SimArm};

const HOME: Joints = [0.0, -0.8, 0.0, -2.2, 0.0, 1.6];
/// Control cycles one measurement needs at a pose.
const SAMPLES: usize = 5;

fn assert_near(actual: Joints, expected: Joints) {
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-6, "{actual:?} != {expected:?}");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> RoplatResult<()> {
    let poses = vec![
        [0.3, -0.5, 0.1, -2.0, 0.0, 1.5],
        [-0.2, -0.6, 0.2, -1.8, 0.1, 1.4],
        [0.1, -0.4, -0.1, -2.1, -0.1, 1.7],
    ];

    // The caller creates the arm, so the caller connects and disconnects it.
    let mut arm = SimArm::new(HOME);
    arm.on_init().await?;

    let (mut arm, cycles) = inspection::inspect(arm, poses.clone()).await?;
    assert_eq!(cycles, vec![SAMPLES; poses.len()]);
    assert_near(arm.joints(), poses[poses.len() - 1]);
    assert!(arm.is_connected(), "the System only uses an arm it did not create");

    arm.on_shutdown().await?;
    println!("inspection: {} poses measured, cycles per pose {cycles:?}", poses.len());
    Ok(())
}
