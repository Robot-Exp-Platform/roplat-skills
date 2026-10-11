//! A 6-axis arm tracks a part on a conveyor until the part leaves its reach.

mod conveyor;
mod nodes;
mod sim;
mod tracking;

use std::time::Duration;

use conveyor::BeltSpec;
use nodes::{YAW, aim};
use roplat::{Lifecycle, RoplatResult};
use sim::{Joints, SimArm};

const HOME: Joints = [0.0, -0.8, 0.0, -2.2, 0.0, 1.6];

#[tokio::main(flavor = "current_thread")]
async fn main() -> RoplatResult<()> {
    // 0.6 m of belt at 0.4 m/s: the part leaves after 150 of the 200 belt ticks.
    let belt = BeltSpec {
        offset: 0.5,
        entry: -0.3,
        exit: 0.3,
        speed: 0.4,
        period: Duration::from_millis(10),
        ticks: 200,
    };

    // The caller creates the arm, so the caller connects and disconnects it.
    let mut arm = SimArm::new(HOME);
    arm.on_init().await?;

    let mut arm = tracking::track_part(arm, belt).await?;
    assert!(arm.is_connected(), "a passed-in arm is used, not closed, by the System");

    let mut expected = HOME;
    expected[YAW] = aim(belt.offset, belt.exit);
    for (actual, target) in arm.joints().iter().zip(expected) {
        assert!((actual - target).abs() < 2e-3, "{:?} != {expected:?}", arm.joints());
    }
    arm.on_shutdown().await?;

    println!("conveyor_tracking: arm yaw {:.4} rad at the part's exit", arm.joints()[YAW]);
    Ok(())
}
