//! One fixture-positioning segment can fail; its parent then parks both arms.
//! The System creates both resources, so it connects and disconnects them once.
pub mod mock_arm;
pub mod nodes;

use robot_behavior::behavior::JointSpace;
use robot_behavior::roplat::MotionNode;
use roplat::RoplatResult;
use roplat::rhythm::once;

use mock_arm::{BenchArm, Goal, Journal};
use nodes::{AttemptStatus, PositionFixture, RetireFixture};

type JointMove = MotionNode<BenchArm, JointSpace<2>>;

#[derive(Debug)]
pub struct Report {
    pub attempt: Option<AttemptStatus>,
    pub left: Goal,
    pub right: Goal,
    pub connected_in_tail: (bool, bool),
}

#[roplat::system]
pub async fn position_and_retire(request: (Goal, Goal), journal: Journal) -> RoplatResult<Report> {
    let mut left = BenchArm::new("left", [0.0; 2], journal.clone());
    let mut right = BenchArm::new("right", [0.0; 2], journal);
    let mut segment = once();
    let mut fixture = PositionFixture;
    let mut retirement = RetireFixture::default();
    let mut park_left = JointMove::new();
    let mut park_right = JointMove::new();

    segment >> {
        &mut left >> fixture.resources.0;
        &mut right >> fixture.resources.1;
        &request >> fixture;
        fixture?;
    };
    segment.outcome >> retirement;
    &mut left >> park_left.resources.0;
    &mut right >> park_right.resources.0;
    retirement.0 >> park_left;
    retirement.1 >> park_right;
    park_left?;
    park_right?;

    Ok(Report {
        attempt: retirement.status().clone(),
        left: left.joints(),
        right: right.joints(),
        connected_in_tail: (left.connected(), right.connected()),
    })
}
