//! Hardware-free pick cycle: one arm visits above → grasp → above → home while a
//! second arm follows it at a fixed joint offset. The caller checks the result.

mod adapters;
mod nodes;
mod pick_and_follow;
mod sim;

use roplat::RoplatResult;
use sim::{Joints, SimArm};

const HOME: Joints = [0.0, -0.8, 0.0, -2.2, 0.0, 1.6];
const ABOVE: Joints = [0.3, -0.5, 0.1, -2.0, 0.0, 1.5];
const GRASP: Joints = [0.3, -0.3, 0.1, -1.9, 0.0, 1.4];
const OFFSET: Joints = [0.0, 0.0, 0.0, 0.0, 0.0, 0.2];

fn shifted(q: Joints, offset: Joints) -> Joints {
    std::array::from_fn(|i| q[i] + offset[i])
}

fn assert_near(name: &str, actual: Joints, expected: Joints) {
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 2e-3, "{name}: {actual:?} != {expected:?}");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> RoplatResult<()> {
    let leader = SimArm::new(HOME);
    let follower = SimArm::new(shifted(HOME, OFFSET));
    let waypoints = vec![ABOVE, GRASP, ABOVE, HOME];

    let (leader, follower) =
        pick_and_follow::pick_and_follow(leader, follower, waypoints, OFFSET).await?;

    println!("leader   final q = {:.4?}", leader.joints());
    println!("follower final q = {:.4?}", follower.joints());
    assert_near("leader", leader.joints(), HOME);
    assert_near("follower", follower.joints(), shifted(HOME, OFFSET));
    println!("pick cycle done: leader back at HOME, follower at HOME + offset");
    Ok(())
}
