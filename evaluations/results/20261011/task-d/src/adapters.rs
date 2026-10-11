//! Glue for today's robot_behavior adapters, where a robot is a value flowing
//! through the graph. Taken from the roplat-development skill assets; these are
//! not robot_behavior API.

use std::future::Future;
use std::marker::PhantomData;

use robot_behavior::{RobotException, RobotResult};
use roplat::rhythm::Rhythm;
use roplat::{Completion, Execution, ExecutionContext, Lifecycle, Node, RoplatError};

fn device_error(error: RobotException) -> RoplatError {
    RoplatError::Io(std::io::Error::other(error))
}

/// A function parameter cannot start a `>>` chain; this node hands it in once.
pub struct Handoff<R>(Option<R>);

impl<R> Handoff<R> {
    pub fn new(robot: R) -> Self {
        Self(Some(robot))
    }
}

impl<R> Lifecycle for Handoff<R> {
    type Error = RoplatError;
}

impl<R: Send + Sync> Node for Handoff<R> {
    type Input = ();
    type Output = RobotResult<R>;

    async fn process(&mut self, (): ()) -> Self::Output {
        self.0
            .take()
            .ok_or_else(|| RobotException::CommandException("robot already handed off".into()))
    }
}

/// Outer task rhythm: each cycle yields the next task together with the robot,
/// and the domain's Feed hands the robot back for the next task.
pub struct TaskSequence<T, R> {
    tasks: std::vec::IntoIter<T>,
    _robot: PhantomData<fn(R) -> R>,
}

impl<T, R> TaskSequence<T, R> {
    pub fn new(tasks: Vec<T>) -> Self {
        Self { tasks: tasks.into_iter(), _robot: PhantomData }
    }
}

impl<T, R> Lifecycle for TaskSequence<T, R> {
    type Error = RoplatError;
}

impl<T: Send, R: Send> Rhythm for TaskSequence<T, R> {
    type Input = RobotResult<R>;
    type Yield = (T, RobotResult<R>);
    type Feed = R;
    type Output = R;

    async fn drive<N, D, Fut>(
        &mut self,
        mut nodes: N,
        mut domain: D,
        robot: RobotResult<R>,
        context: ExecutionContext,
    ) -> (Execution<R>, N)
    where
        N: Send,
        D: FnMut(N, Self::Yield, ExecutionContext) -> Fut + Send,
        Fut: Future<Output = (Execution<R>, N)> + Send,
    {
        let mut robot = match robot {
            Ok(robot) => robot,
            Err(error) => {
                context.request_stop();
                return (Err(device_error(error)), nodes);
            }
        };
        for task in self.tasks.by_ref() {
            if context.is_stopping() {
                return (Ok(Completion::Stopped), nodes);
            }
            let (execution, returned) = domain(nodes, (task, Ok(robot)), context.clone()).await;
            nodes = returned;
            robot = match execution {
                Ok(Completion::Completed(robot)) => robot,
                Ok(Completion::Stopped) => {
                    context.request_stop();
                    return (Ok(Completion::Stopped), nodes);
                }
                Err(error) => {
                    context.request_stop();
                    return (Err(error), nodes);
                }
            };
        }
        (Ok(Completion::Completed(robot)), nodes)
    }
}
