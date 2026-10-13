use roplat::RoplatResult;
use robot_behavior::{JointSpace, roplat::MotionNode};
use roplat_forward_test::mock_arm::BenchArm;
#[roplat::system]
async fn return_external_arm(mut arm: BenchArm) -> RoplatResult<BenchArm> {
    let mut move_home = MotionNode::<BenchArm, JointSpace<2>>::new();
    let target = [0.0; 2];
    &mut arm >> move_home.resources.0;
    &target >> move_home;
    move_home?;
    Ok(arm)
}
fn main() {}
