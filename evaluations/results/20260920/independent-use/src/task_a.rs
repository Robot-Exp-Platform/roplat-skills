use roplat::rhythm::Rhythm;
use roplat::{Completion, Execution, ExecutionContext, Lifecycle, Node, RoplatError, RoplatResult};
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
#[derive(Default)]
struct Counts {
    made: AtomicUsize,
    initialized: AtomicUsize,
    closed: AtomicUsize,
    dropped: AtomicUsize,
}
type Trace = Arc<Counts>;
struct Seed;
impl Lifecycle for Seed {
    type Error = RoplatError;
}
impl Node for Seed {
    type Input = ();
    type Output = u64;
    async fn process(&mut self, (): ()) -> u64 {
        1
    }
}
struct Worker {
    calls: u64,
    initialized: usize,
    closed: usize,
}
impl Lifecycle for Worker {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.initialized += 1;
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.closed += 1;
        Ok(())
    }
}
impl Node for Worker {
    type Input = u64;
    type Output = u64;
    async fn process(&mut self, input: u64) -> u64 {
        self.calls += 1;
        input + self.calls
    }
}
struct Temporary {
    trace: Trace,
}
impl Temporary {
    fn new(trace: Trace) -> Self {
        trace.made.fetch_add(1, Ordering::Relaxed);
        Self { trace }
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        self.trace.dropped.fetch_add(1, Ordering::Relaxed);
    }
}
impl Lifecycle for Temporary {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.trace.initialized.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.trace.closed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
impl Node for Temporary {
    type Input = u64;
    type Output = u64;
    async fn process(&mut self, input: u64) -> u64 {
        input
    }
}
struct SumRhythm {
    cycles: u64,
    offset: u64,
    drives: usize,
}
impl Lifecycle for SumRhythm {
    type Error = RoplatError;
}
impl Rhythm for SumRhythm {
    type Input = u64;
    type Yield = u64;
    type Feed = u64;
    type Output = u64;
    async fn drive<N, F, Fut>(
        &mut self,
        mut nodes: N,
        mut operation: F,
        input: u64,
        context: ExecutionContext,
    ) -> (Execution<u64>, N)
    where
        N: Send,
        F: FnMut(N, u64, ExecutionContext) -> Fut + Send,
        Fut: Future<Output = (Execution<u64>, N)> + Send,
    {
        self.drives += 1;
        let mut total = self.offset;
        for tick in 0..self.cycles {
            if context.is_stopping() {
                return (Ok(Completion::Stopped), nodes);
            }
            let (outcome, returned) = operation(nodes, input + tick, context.clone()).await;
            nodes = returned;
            match outcome {
                Ok(Completion::Completed(feed)) => total += feed,
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
        (Ok(Completion::Completed(total)), nodes)
    }
}
#[roplat::system]
async fn sample_layers(
    seed: &mut Seed,
    outer: &mut SumRhythm,
    calibration: &mut SumRhythm,
    sampling: &mut SumRhythm,
    worker: &mut Worker,
    trace: &Trace,
) -> RoplatResult<u64> {
    seed >> outer
        >> |outer_value| {
            outer_value
                >> calibration
                >> |calibrated_value| {
                    calibrated_value
                        >> sampling
                        >> |sample_value| {
                            let mut transient = Temporary::new(trace.clone());
                            sample_value >> worker >> transient
                        }
                }
        };
    Ok(outer.output)
}
pub async fn verify() -> RoplatResult<()> {
    let trace = Trace::default();
    let mut seed = Seed;
    let mut outer = SumRhythm {
        cycles: 2,
        offset: 10000,
        drives: 0,
    };
    let mut calibration = SumRhythm {
        cycles: 3,
        offset: 1000,
        drives: 0,
    };
    let mut sampling = SumRhythm {
        cycles: 2,
        offset: 100,
        drives: 0,
    };
    let mut worker = Worker {
        calls: 0,
        initialized: 0,
        closed: 0,
    };
    seed.on_init().await?;
    outer.on_init().await?;
    calibration.on_init().await?;
    sampling.on_init().await?;
    worker.on_init().await?;
    let first = sample_layers(
        &mut seed,
        &mut outer,
        &mut calibration,
        &mut sampling,
        &mut worker,
        &trace,
    )
    .await?;
    assert_eq!(first, 12714);
    assert_eq!(worker.calls, 12);
    let second = sample_layers(
        &mut seed,
        &mut outer,
        &mut calibration,
        &mut sampling,
        &mut worker,
        &trace,
    )
    .await?;
    assert_eq!(second, 12858);
    assert_eq!(worker.calls, 24);
    assert_eq!(
        (outer.drives, calibration.drives, sampling.drives),
        (2, 4, 12)
    );
    assert_eq!((worker.initialized, worker.closed), (1, 0));
    worker.on_shutdown().await?;
    sampling.on_shutdown().await?;
    calibration.on_shutdown().await?;
    outer.on_shutdown().await?;
    seed.on_shutdown().await?;
    assert_eq!((worker.initialized, worker.closed), (1, 1));
    for value in [
        &trace.made,
        &trace.initialized,
        &trace.closed,
        &trace.dropped,
    ] {
        assert_eq!(value.load(Ordering::Relaxed), 24);
    }
    println!("A: outputs {first}/{second}; calls=24; transient create/init/shutdown/drop=24");
    Ok(())
}
