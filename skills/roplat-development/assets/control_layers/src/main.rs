//! Real System DSL: three finite rhythms, retained external state, local
//! lifecycle, complete child Output, and explicit business-error propagation.

use std::future::Future;
use std::sync::{Arc, Mutex};

use roplat::rhythm::Rhythm;
use roplat::{Completion, Execution, ExecutionContext, Lifecycle, Node, RoplatError, RoplatResult};

type Trace = Arc<Mutex<Vec<String>>>;

struct Activity {
    name: &'static str,
    trace: Trace,
    active: bool,
}
impl Activity {
    fn new(name: &'static str, trace: &Trace) -> Self {
        Self {
            name,
            trace: trace.clone(),
            active: false,
        }
    }
    fn record(&self, action: &str) {
        self.trace
            .lock()
            .unwrap()
            .push(format!("{}:{action}", self.name));
    }
    fn activate(&mut self) {
        assert!(!self.active);
        self.active = true;
        self.record("init");
    }
    fn close(&mut self) {
        assert!(self.active);
        self.active = false;
        self.record("shutdown");
    }
}

struct Source {
    activity: Activity,
    value: i64,
    fail: bool,
}
impl Lifecycle for Source {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.activity.activate();
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.activity.close();
        Ok(())
    }
}
impl Node for Source {
    type Input = ();
    type Output = RoplatResult<i64>;
    async fn process(&mut self, (): ()) -> Self::Output {
        assert!(self.activity.active);
        self.activity.record("process");
        if self.fail {
            Err(RoplatError::NodeProcessing(
                "requested source failure".into(),
            ))
        } else {
            Ok(self.value)
        }
    }
}

struct Worker {
    activity: Activity,
    calls: usize,
    increment: i64,
}
impl Worker {
    fn new(name: &'static str, trace: &Trace, increment: i64) -> Self {
        Self {
            activity: Activity::new(name, trace),
            calls: 0,
            increment,
        }
    }
}
impl Lifecycle for Worker {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.activity.activate();
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.activity.close();
        Ok(())
    }
}
impl Node for Worker {
    type Input = i64;
    type Output = i64;
    async fn process(&mut self, value: i64) -> i64 {
        assert!(self.activity.active);
        self.calls += 1;
        self.activity.record("process");
        value + self.increment
    }
}

/// Each drive starts from Input, yields Input + tick twice, and returns
/// 1000 + sum(Feed). Output is deliberately different from any single Feed.
struct FoldRhythm {
    activity: Activity,
    drives: usize,
}
impl FoldRhythm {
    fn new(name: &'static str, trace: &Trace) -> Self {
        Self {
            activity: Activity::new(name, trace),
            drives: 0,
        }
    }
}
impl Lifecycle for FoldRhythm {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.activity.activate();
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.activity.close();
        Ok(())
    }
}
impl Rhythm for FoldRhythm {
    type Input = i64;
    type Output = i64;
    type Yield = i64;
    type Feed = i64;

    async fn drive<N, F, Fut>(
        &mut self,
        mut nodes: N,
        mut domain: F,
        input: i64,
        context: ExecutionContext,
    ) -> (Execution<i64>, N)
    where
        N: Send,
        F: FnMut(N, i64, ExecutionContext) -> Fut + Send,
        Fut: Future<Output = (Execution<i64>, N)> + Send,
    {
        assert!(self.activity.active);
        self.drives += 1;
        self.activity.record("drive");
        let mut output = 1000;
        for tick in 0..2 {
            if context.is_stopping() {
                return (Ok(Completion::Stopped), nodes);
            }
            // The callback owns or borrows N. Recover it before handling status.
            // Do not timeout/drop this future to "recover" the nodes.
            let (execution, returned) = domain(nodes, input + tick, context.clone()).await;
            nodes = returned;
            match execution {
                Ok(Completion::Completed(feed)) => output += feed,
                exit => {
                    context.request_stop();
                    return (exit, nodes);
                }
            }
        }
        if context.is_stopping() {
            (Ok(Completion::Stopped), nodes)
        } else {
            (Ok(Completion::Completed(output)), nodes)
        }
    }
}

#[roplat::system]
async fn three_layers(
    outer: &mut FoldRhythm,
    middle: &mut FoldRhythm,
    inner: &mut FoldRhythm,
    worker: &mut Worker,
    trace: &Trace,
) -> RoplatResult<i64> {
    // Created in this System: its lifecycle belongs here, once per System call.
    let mut source = Source {
        activity: Activity::new("source", trace),
        value: 1,
        fail: false,
    };

    source.output?
        >> outer
        >> |outer_yield| {
            outer_yield
                >> middle
                >> |middle_yield| {
                    middle_yield
                        >> inner
                        >> |inner_yield| {
                            // Created inside the innermost cycle: eight instances
                            // per outer drive, each initialized and closed once.
                            let mut local = Worker::new("local", trace, 0);
                            inner_yield >> worker >> local
                        }
                }
        };

    // Explicit Rust tail receives the complete outer drive Output.
    Ok(outer.output)
}

#[roplat::system]
async fn failure_cleanup(trace: Trace, fail: bool) -> RoplatResult<i64> {
    let mut source = Source {
        activity: Activity::new("source", &trace),
        value: 5,
        fail,
    };
    let mut next = Worker::new("next", &trace, 1);

    // Source::Output is a business Result. The explicit ? promotes its failure.
    source.output? >> next;

    Ok(next.output)
}

fn count(trace: &Trace, event: &str) -> usize {
    trace
        .lock()
        .unwrap()
        .iter()
        .filter(|entry| entry.as_str() == event)
        .count()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> RoplatResult<()> {
    let trace = Trace::default();
    let mut outer = FoldRhythm::new("outer", &trace);
    let mut middle = FoldRhythm::new("middle", &trace);
    let mut inner = FoldRhythm::new("inner", &trace);
    let mut worker = Worker::new("worker", &trace, 1);
    // These objects are created outside three_layers; this creator activates them.
    outer.on_init().await?;
    middle.on_init().await?;
    inner.on_init().await?;
    worker.on_init().await?;

    for run in 1..=2 {
        let output = three_layers(&mut outer, &mut middle, &mut inner, &mut worker, &trace).await?;
        // inner(x) = 1000 + (x+1) + (x+2) = 1003+2x
        // middle(x) = 1000 + inner(x) + inner(x+1) = 3008+4x
        // outer(1) = 1000 + middle(1) + middle(2) = 7028
        assert_eq!(output, 7028);
        assert_eq!(worker.calls, run * 8);
        assert_eq!(
            (outer.drives, middle.drives, inner.drives),
            (run, run * 2, run * 4)
        );
        assert_eq!(count(&trace, "worker:init"), 1);
        assert_eq!(count(&trace, "worker:shutdown"), 0);
        assert_eq!(count(&trace, "local:init"), run * 8);
        assert_eq!(count(&trace, "local:shutdown"), run * 8);
        assert_eq!(count(&trace, "source:init"), run);
        assert_eq!(count(&trace, "source:shutdown"), run);
    }
    worker.on_shutdown().await?;
    inner.on_shutdown().await?;
    middle.on_shutdown().await?;
    outer.on_shutdown().await?;
    assert_eq!(count(&trace, "worker:shutdown"), 1);

    let failure_trace = Trace::default();
    assert!(failure_cleanup(failure_trace.clone(), true).await.is_err());
    assert_eq!(
        failure_trace.lock().unwrap().as_slice(),
        [
            "source:init",
            "next:init",
            "source:process",
            "next:shutdown",
            "source:shutdown"
        ]
    );
    assert_eq!(failure_cleanup(Trace::default(), false).await?, 6);
    println!(
        "Passed: three rhythm layers, full drive outputs, retained state, local lifecycle, failure cleanup."
    );
    Ok(())
}
