use roplat::comm::{FrameBuffer, Latest, Publisher, SharedFrame, Subscriber};
use roplat::rhythm::CountRhythm;
use roplat::{Lifecycle, Node, RoplatError, RoplatResult};
type State = (usize, SharedFrame);
struct Publish {
    output: Publisher<State>,
    calls: usize,
}
impl Lifecycle for Publish {
    type Error = RoplatError;
}
impl Node for Publish {
    type Input = usize;
    type Output = ();
    async fn process(&mut self, tick: usize) {
        let mut frame = FrameBuffer::new(4096);
        frame.as_mut_slice().fill((tick + 1) as u8);
        self.output.publish((tick + 1, frame.seal()));
        self.calls += 1;
        tokio::task::yield_now().await;
    }
}
struct Sample {
    input: Subscriber<State>,
    observed: Vec<usize>,
    calls: usize,
}
impl Lifecycle for Sample {
    type Error = RoplatError;
}
impl Node for Sample {
    type Input = usize;
    type Output = ();
    async fn process(&mut self, _tick: usize) {
        tokio::task::yield_now().await;
        if let Some((version, frame)) = self.input.latest() {
            assert_eq!(frame.len(), 4096);
            if *version > 0 {
                assert!(frame.as_slice().iter().all(|byte| *byte == *version as u8));
            }
            self.observed.push(*version);
        }
        self.calls += 1;
    }
}
#[roplat::system]
async fn independent_controls(
    mut publisher: Publish,
    mut sampler: Sample,
) -> RoplatResult<(Publish, Sample)> {
    let mut writing = CountRhythm::new(8);
    let mut reading = CountRhythm::new(5);
    writing
        >> |tick| {
            tick >> publisher;
        };
    reading
        >> |tick| {
            tick >> sampler;
        };
    Ok((publisher, sampler))
}
pub async fn verify() -> RoplatResult<()> {
    let state = Latest::<State>::new(2);
    let mut output = state.publisher();
    let input = state.subscriber(0);
    let mut final_reader = state.subscriber(1);
    let mut initial = FrameBuffer::new(4096);
    initial.as_mut_slice().fill(0xaa);
    output.publish((0, initial.seal()));
    let old_snapshot = final_reader.snapshot().unwrap().1;
    let mut publisher = Publish { output, calls: 0 };
    let mut sampler = Sample {
        input,
        observed: Vec::new(),
        calls: 0,
    };
    publisher.on_init().await?;
    sampler.on_init().await?;
    let (mut publisher, mut sampler) = independent_controls(publisher, sampler).await?;
    assert_eq!((publisher.calls, sampler.calls), (8, 5));
    assert!(sampler.observed.windows(2).all(|pair| pair[0] <= pair[1]));
    let (version, current) = final_reader.snapshot().unwrap();
    assert_eq!(version, 8);
    assert!(current.as_slice().iter().all(|byte| *byte == 8));
    assert!(old_snapshot.as_slice().iter().all(|byte| *byte == 0xaa));
    assert!(!old_snapshot.ptr_eq(&current));
    sampler.on_shutdown().await?;
    publisher.on_shutdown().await?;
    println!(
        "C: 8/5 calls; sampled {:?}; final=8; old4KiB intact",
        sampler.observed
    );
    Ok(())
}
