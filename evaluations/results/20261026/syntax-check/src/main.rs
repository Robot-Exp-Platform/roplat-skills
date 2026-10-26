//! Finite publication-only syntax checks. No hardware, source patches, or network I/O.
use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
    time::Duration,
};

use robot_behavior::roplat::{ControlRhythm, MotionNode};
use robot_behavior::{
    ControlStep, ControlWith, JointPositionControl, JointSample, JointSpace, JointState, Joints,
    MoveTo, Robot, RobotException, RobotResult,
};
use roplat::rhythm::{IntoHard, create_count_rhythm, once};
use roplat::{Lifecycle, RoplatError, RoplatResult};

type Events = Arc<Mutex<Vec<&'static str>>>;
type Threads = Arc<Mutex<Vec<ThreadId>>>;
type Samples = Arc<Mutex<Vec<(usize, ThreadId)>>>;

struct ProbeArm {
    q: [f64; 1],
    fail_motion: bool,
    events: Events,
    control_threads: Threads,
}

impl ProbeArm {
    fn observe(&self) -> JointState<1> {
        JointState::from_meas(JointSample {
            q: Some(self.q),
            ..Default::default()
        })
    }
}

impl Lifecycle for ProbeArm {
    type Error = RoplatError;
}
impl Robot for ProbeArm {
    type State = JointState<1>;
    const CONTROL_PERIOD: f64 = 0.001;
    fn version() -> String {
        "syntax-only-probe".into()
    }
    fn read_state(&mut self) -> RobotResult<Self::State> {
        Ok(self.observe())
    }
}
impl Joints<1> for ProbeArm {
    const JOINT_MIN: [f64; 1] = [-3.0];
    const JOINT_MAX: [f64; 1] = [3.0];
}
impl MoveTo<JointSpace<1>> for ProbeArm {
    fn move_to(&mut self, target: [f64; 1]) -> RobotResult<()> {
        self.events.lock().unwrap().push("motion");
        if self.fail_motion {
            return Err(RobotException::CommandException(
                "expected injected motion failure".into(),
            ));
        }
        self.q = target;
        Ok(())
    }
}
impl ControlWith<JointPositionControl<1>> for ProbeArm {
    fn hold_command(observation: &JointState<1>) -> [f64; 1] {
        observation.meas.q.unwrap()
    }

    fn control_with_flow<F>(&mut self, mut callback: F) -> RobotResult<()>
    where
        F: FnMut(JointState<1>, Duration) -> ControlStep<[f64; 1]>,
    {
        self.events.lock().unwrap().push("control");
        self.control_threads
            .lock()
            .unwrap()
            .push(thread::current().id());
        match callback(self.observe(), Duration::from_millis(1)) {
            ControlFlow::Continue((command, true)) => {
                self.q = command;
                Ok(())
            }
            ControlFlow::Break(()) => Ok(()),
            ControlFlow::Continue((_, false)) => Err(RobotException::CommandException(
                "the finite probe requires completion after exactly one callback".into(),
            )),
        }
    }
}

#[roplat::system]
async fn motion_then_control(mut arm: ProbeArm, target: [f64; 1]) -> RoplatResult<ProbeArm> {
    let mut motion = MotionNode::<ProbeArm, JointSpace<1>>::new();
    let mut control = ControlRhythm::<ProbeArm, JointPositionControl<1>>::new();
    let mut servo = roplat::node::map(|state: JointState<1>| (state.meas.q.unwrap(), true));

    &mut arm >> motion.resources.0;
    &target >> motion;
    motion?;
    motion >> control.trigger;
    &mut arm >> control.resources.0;
    control
        >> |(state, _dt)| {
            state >> servo;
            servo
        };

    Ok(arm)
}

#[roplat::system]
async fn hard_count(mut arm: ProbeArm, samples: Samples) -> RoplatResult<ProbeArm> {
    let (count_handle, mut camera) = create_count_rhythm(2);
    let mut infer = once::<usize, (usize, ThreadId)>().hard();
    let mut control = ControlRhythm::<ProbeArm, JointPositionControl<1>>::new().hard();
    let mut detector = roplat::node::map(|frame: usize| (frame + 10, thread::current().id()));
    let mut tracker =
        roplat::node::map(move |sample: (usize, ThreadId)| samples.lock().unwrap().push(sample));
    let mut law = roplat::node::map(|state: JointState<1>| (state.meas.q.unwrap(), true));
    count_handle.set_times(3);

    camera
        >> |frame| {
            frame
                >> infer
                >> move |frame| {
                    frame >> detector;
                    detector
                };
            infer.output >> tracker;
        };
    &mut arm >> control.resources.0;
    control
        >> move |(state, _dt)| {
            state >> law;
            law
        };

    Ok(arm)
}

fn probe(fail_motion: bool) -> (ProbeArm, Events, Threads) {
    let events = Events::default();
    let threads = Threads::default();
    (
        ProbeArm {
            q: [0.0],
            fail_motion,
            events: events.clone(),
            control_threads: threads.clone(),
        },
        events,
        threads,
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> RoplatResult<()> {
    let (arm, events, _) = probe(false);
    let arm = motion_then_control(arm, [1.25]).await?;
    assert_eq!(*events.lock().unwrap(), vec!["motion", "control"]);
    assert_eq!(arm.q, [1.25]);
    println!("PASS resource success: motion then control, one callback, q=1.25");

    let (arm, events, threads) = probe(true);
    let result = motion_then_control(arm, [2.0]).await;
    println!(
        "OBSERVED standalone-question failure: is_err={}, events={:?}, control_entries={}",
        result.is_err(),
        events.lock().unwrap(),
        threads.lock().unwrap().len()
    );
    assert!(result.is_err());
    assert_eq!(*events.lock().unwrap(), vec!["motion"]);
    assert!(threads.lock().unwrap().is_empty());
    println!("PASS resource failure: motion? propagates error; control is never entered");

    let (arm, events, control_threads) = probe(false);
    let samples = Samples::default();
    let main_thread = thread::current().id();
    let _arm = hard_count(arm, samples.clone()).await?;
    let samples = samples.lock().unwrap();
    assert_eq!(
        samples.iter().map(|(frame, _)| *frame).collect::<Vec<_>>(),
        vec![10, 11, 12]
    );
    assert!(samples.iter().all(|(_, id)| *id != main_thread));
    assert_eq!(*events.lock().unwrap(), vec!["control"]);
    let control_threads = control_threads.lock().unwrap();
    assert_eq!(control_threads.len(), 1);
    assert_ne!(control_threads[0], main_thread);
    println!("PASS count factory: destructured handle changes 2 to 3 bounded cycles");
    println!(
        "PASS hard graph: header declarations, contiguous graph, planning and control run on hard threads"
    );
    Ok(())
}
