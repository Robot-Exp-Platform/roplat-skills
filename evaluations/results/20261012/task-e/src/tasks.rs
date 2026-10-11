//! Task-layer rhythm: hands out one task per cycle and collects what each cycle
//! reports, so the caller gets per-task results with the complete drive.

use std::future::Future;
use std::marker::PhantomData;

use roplat::rhythm::Rhythm;
use roplat::{Completion, Execution, ExecutionContext, Lifecycle, RoplatError};

pub struct TaskList<T, R> {
    tasks: Vec<T>,
    _results: PhantomData<fn() -> R>,
}

impl<T, R> TaskList<T, R> {
    pub fn new(tasks: Vec<T>) -> Self {
        Self { tasks, _results: PhantomData }
    }
}

impl<T, R> Lifecycle for TaskList<T, R> {
    type Error = RoplatError;
}

impl<T, R> Rhythm for TaskList<T, R>
where
    T: Clone + Send + Sync,
    R: Send,
{
    type Input = ();
    type Yield = T;
    type Feed = R;
    type Output = Vec<R>;

    async fn drive<N, F, Fut>(
        &mut self,
        mut nodes: N,
        mut domain: F,
        (): (),
        context: ExecutionContext,
    ) -> (Execution<Vec<R>>, N)
    where
        N: Send,
        F: FnMut(N, T, ExecutionContext) -> Fut + Send,
        Fut: Future<Output = (Execution<R>, N)> + Send,
    {
        let mut results = Vec::with_capacity(self.tasks.len());
        for task in &self.tasks {
            if context.is_stopping() {
                return (Ok(Completion::Stopped), nodes);
            }
            // Recover the nodes before looking at how the cycle ended.
            let (execution, returned) = domain(nodes, task.clone(), context.clone()).await;
            nodes = returned;
            match execution {
                Ok(Completion::Completed(result)) => results.push(result),
                Ok(Completion::Stopped) => {
                    context.request_stop();
                    return (Ok(Completion::Stopped), nodes);
                }
                Err(error) => {
                    context.request_stop();
                    return (Err(error), nodes);
                }
            }
        }
        (Ok(Completion::Completed(results)), nodes)
    }
}
