use roplat::rhythm::Rhythm;
use roplat::{Completion, Execution, ExecutionContext, Lifecycle, Node, RoplatError, RoplatResult};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};
type Log = Arc<Mutex<Vec<String>>>;
struct Bad {
    log: Log,
}
impl Lifecycle for Bad {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("bad.init".into());
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("bad.shutdown".into());
        Ok(())
    }
}
impl Node for Bad {
    type Input = ();
    type Output = RoplatResult<u64>;
    async fn process(&mut self, (): ()) -> Self::Output {
        self.log.lock().unwrap().push("bad.process".into());
        Err(RoplatError::Other("quality".into()))
    }
}
struct Handler {
    log: Log,
}
impl Lifecycle for Handler {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("handler.init".into());
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("handler.shutdown".into());
        Ok(())
    }
}
impl Node for Handler {
    type Input = RoplatResult<u64>;
    type Output = u64;
    async fn process(&mut self, input: Self::Input) -> u64 {
        self.log.lock().unwrap().push("handler.process".into());
        input.unwrap_or(41)
    }
}
struct After {
    log: Log,
}
impl Lifecycle for After {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("after.init".into());
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("after.shutdown".into());
        Ok(())
    }
}
impl Node for After {
    type Input = u64;
    type Output = u64;
    async fn process(&mut self, input: u64) -> u64 {
        self.log.lock().unwrap().push("after.process".into());
        input
    }
}
struct Recover {
    log: Log,
}
impl Lifecycle for Recover {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("recover.init".into());
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("recover.shutdown".into());
        Ok(())
    }
}
impl Node for Recover {
    type Input = Execution<u64>;
    type Output = u64;
    async fn process(&mut self, input: Self::Input) -> u64 {
        self.log.lock().unwrap().push("recover.process".into());
        assert!(matches!(input,Err(RoplatError::Other(ref text)) if text=="quality"));
        99
    }
}
struct Once {
    log: Log,
}
impl Lifecycle for Once {
    type Error = RoplatError;
    async fn on_init(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("once.init".into());
        Ok(())
    }
    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.log.lock().unwrap().push("once.shutdown".into());
        Ok(())
    }
}
impl Rhythm for Once {
    type Input = ();
    type Yield = ();
    type Feed = u64;
    type Output = u64;
    async fn drive<N, F, Fut>(
        &mut self,
        nodes: N,
        mut op: F,
        (): (),
        context: ExecutionContext,
    ) -> (Execution<u64>, N)
    where
        N: Send,
        F: FnMut(N, (), ExecutionContext) -> Fut + Send,
        Fut: Future<Output = (Execution<u64>, N)> + Send,
    {
        if context.is_stopping() {
            return (Ok(Completion::Stopped), nodes);
        }
        let (result, nodes) = op(nodes, (), context.clone()).await;
        if !matches!(result, Ok(Completion::Completed(_))) {
            context.request_stop();
        }
        (result, nodes)
    }
}
#[roplat::system]
async fn business_error(log: &Log) -> RoplatResult<u64> {
    let mut bad = Bad { log: log.clone() };
    let mut handler = Handler { log: log.clone() };
    bad >> handler;
    Ok(handler.output)
}
#[roplat::system]
async fn promoted_error(log: &Log) -> RoplatResult<u64> {
    let mut bad = Bad { log: log.clone() };
    let mut after = After { log: log.clone() };
    bad.output? >> after;
    Ok(after.output)
}
#[roplat::system]
async fn recovered_child(log: &Log) -> RoplatResult<u64> {
    let mut bad = Bad { log: log.clone() };
    let mut child = Once { log: log.clone() };
    let mut recover = Recover { log: log.clone() };
    child >> { bad.output? };
    child.outcome >> recover;
    Ok(recover.output)
}
pub async fn verify() -> RoplatResult<()> {
    let log = Log::default();
    assert_eq!(business_error(&log).await?, 41);
    assert_eq!(
        *log.lock().unwrap(),
        [
            "bad.init",
            "handler.init",
            "bad.process",
            "handler.process",
            "handler.shutdown",
            "bad.shutdown"
        ]
    );
    log.lock().unwrap().clear();
    assert!(
        matches!(promoted_error(&log).await,Err(RoplatError::Other(ref text)) if text=="quality")
    );
    assert_eq!(
        *log.lock().unwrap(),
        [
            "bad.init",
            "after.init",
            "bad.process",
            "after.shutdown",
            "bad.shutdown"
        ]
    );
    log.lock().unwrap().clear();
    assert_eq!(recovered_child(&log).await?, 99);
    assert_eq!(
        *log.lock().unwrap(),
        [
            "bad.init",
            "once.init",
            "recover.init",
            "bad.process",
            "recover.process",
            "recover.shutdown",
            "once.shutdown",
            "bad.shutdown"
        ]
    );
    println!("B: business Err=41; promoted Err skips consumer; child outcome=99; lifecycle exact");
    Ok(())
}
