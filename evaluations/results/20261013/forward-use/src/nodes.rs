use robot_behavior::RobotResult;
use robot_behavior::behavior::*;
use roplat::{Completion, Execution, Lifecycle, Node, RoplatError};

use crate::mock_arm::{BenchArm, Goal};

/// Position two fixture arms. A rejected second motion does not undo the first.
pub struct PositionFixture;

impl Lifecycle for PositionFixture {
    type Error = RoplatError;
}

impl Node for PositionFixture {
    type Resources = (BenchArm, BenchArm);
    type Input = (Goal, Goal);
    type Output = RobotResult<()>;

    async fn process<'call>(
        &'call mut self,
        (left_goal, right_goal): Self::Input,
        (left, right): (&'call mut BenchArm, &'call mut BenchArm),
    ) -> Self::Output {
        left.move_to::<JointSpace<2>>(left_goal)?;
        right.move_to::<JointSpace<2>>(right_goal)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptStatus {
    Completed,
    Failed(String),
    Stopped,
}

/// A deterministic mock recovery policy: retire the fixture and park both arms.
#[derive(Default)]
pub struct RetireFixture {
    status: Option<AttemptStatus>,
}

impl RetireFixture {
    pub fn status(&self) -> &Option<AttemptStatus> {
        &self.status
    }
}

impl Lifecycle for RetireFixture {
    type Error = RoplatError;
}

impl Node for RetireFixture {
    type Resources = ();
    type Input = Execution<()>;
    type Output = (Goal, Goal);

    async fn process(&mut self, attempt: Self::Input, _: ()) -> Self::Output {
        self.status = Some(match attempt {
            Ok(Completion::Completed(())) => AttemptStatus::Completed,
            Ok(Completion::Stopped) => AttemptStatus::Stopped,
            Err(error) => AttemptStatus::Failed(error.to_string()),
        });
        ([-0.25; 2], [0.25; 2])
    }
}
