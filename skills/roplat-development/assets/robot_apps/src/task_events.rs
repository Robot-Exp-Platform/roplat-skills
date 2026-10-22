//! Event-driven tasks. An operator console outside the System sends commands
//! through an event channel, and each command is one cycle of the task domain.
//! Three checks decide what reaches the arm, each in the form that fits it:
//!
//! - a match guard rejects targets outside the joint limits: plain Rust on the
//!   command itself, no node involved;
//! - an `if` on a node's answer skips targets the collision checker blocks: the
//!   condition is computed by a node, so it is a graph dependency;
//! - a retry rhythm repeats a motion the driver reports as failed; a motion that
//!   still fails is reported as business data and the console carries on.
//!
//! Refresh requests are a signal: presses before the panel's next cycle merge
//! into one. Both triggers live outside the System; once the console drops
//! them and the queued work is done, both rhythms finish and the System ends.

use std::ops::Range;
use std::time::Duration;

use robot_behavior::roplat::MotionNode;
use robot_behavior::{JointSpace, Joints as JointLimits, RobotException, RobotResult};
use roplat::comm::{Latest, Publisher, Subscriber};
use roplat::rhythm::{EventRhythm, SignalRhythm, create_event_channel, create_signal};
use roplat::{Lifecycle, Node, RoplatError, RoplatResult};

use crate::sim::{DOF, Joints, SimArm};

type MoveJoints = MotionNode<SimArm, JointSpace<DOF>>;

#[derive(Clone, Copy, Debug)]
pub enum Command {
    /// Move to a joint target.
    Goto(Joints),
    /// Stay still for a while, e.g. while a tool works.
    Dwell(Duration),
}

/// What the operator panel shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    pub joints: Joints,
    pub moved: bool,
    pub motions: usize,
}

/// What the caller gets back besides the arm.
#[derive(Debug)]
pub struct Outcome {
    pub attempts: usize,
    pub out_of_reach: Vec<Joints>,
    pub blocked: Vec<Joints>,
    pub panel: Vec<Option<Status>>,
}

fn within_limits(target: &Joints) -> bool {
    target.iter().enumerate().all(|(i, q)| {
        (<SimArm as JointLimits<DOF>>::JOINT_MIN[i]..=<SimArm as JointLimits<DOF>>::JOINT_MAX[i])
            .contains(q)
    })
}

/// Collision checker. It owns the obstacle model, so its answer comes from a
/// node rather than from a guard.
pub struct Clearance {
    /// Base-joint angles that would sweep through a fixture.
    keep_out: Range<f64>,
}

impl Clearance {
    pub fn new(keep_out: Range<f64>) -> Self {
        Self { keep_out }
    }
}

impl Lifecycle for Clearance {
    type Error = RoplatError;
}

impl Node for Clearance {
    type Resources = ();
    type Input = Joints;
    type Output = bool;

    async fn process(&mut self, target: Joints, _: ()) -> bool {
        !self.keep_out.contains(&target[0])
    }
}

/// Collects targets refused for one reason.
#[derive(Default)]
pub struct Refusals {
    pub targets: Vec<Joints>,
}

impl Lifecycle for Refusals {
    type Error = RoplatError;
}

impl Node for Refusals {
    type Resources = ();
    type Input = Joints;
    type Output = ();

    async fn process(&mut self, target: Joints, _: ()) {
        self.targets.push(target);
    }
}

/// Waits without touching the arm.
pub struct Dwell;

impl Lifecycle for Dwell {
    type Error = RoplatError;
}

impl Node for Dwell {
    type Resources = ();
    type Input = Duration;
    type Output = ();

    async fn process(&mut self, time: Duration, _: ()) {
        tokio::time::sleep(time).await;
    }
}

/// Reads the arm after a motion and publishes the panel status.
pub struct Report {
    board: Publisher<Status>,
    motions: usize,
}

impl Report {
    pub fn new(board: Publisher<Status>) -> Self {
        Self { board, motions: 0 }
    }
}

impl Lifecycle for Report {
    type Error = RoplatError;
}

impl Node for Report {
    type Resources = (SimArm,);
    type Input = RobotResult<()>;
    type Output = ();

    async fn process<'call>(&'call mut self, moved: RobotResult<()>, (arm,): (&'call mut SimArm,)) {
        self.motions += 1;
        self.board.publish(Status {
            joints: arm.joints(),
            moved: moved.is_ok(),
            motions: self.motions,
        });
    }
}

/// The operator panel: shows the newest status when asked to refresh.
pub struct Panel {
    board: Subscriber<Status>,
    pub shown: Vec<Option<Status>>,
}

impl Panel {
    pub fn new(board: Subscriber<Status>) -> Self {
        Self {
            board,
            shown: Vec::new(),
        }
    }
}

impl Lifecycle for Panel {
    type Error = RoplatError;
}

impl Node for Panel {
    type Resources = ();
    type Input = ();
    type Output = ();

    async fn process(&mut self, (): (), _: ()) {
        self.shown.push(self.board.latest().copied());
    }
}

#[roplat::system]
pub async fn serve(
    mut arm: SimArm,
    mut commands: EventRhythm<Command>,
    mut refresh: SignalRhythm,
    keep_out: Range<f64>,
) -> RoplatResult<(SimArm, Outcome)> {
    let board = Latest::<Status>::new(1);
    let mut clear = Clearance::new(keep_out);
    let mut retry = roplat::rhythm::retry::<Joints, (), RobotException>(3);
    let mut motion = MoveJoints::new();
    let mut report = Report::new(board.publisher());
    let mut out_of_reach = Refusals::default();
    let mut blocked = Refusals::default();
    let mut dwell = Dwell;
    let mut panel = Panel::new(board.subscriber(0));

    commands
        >> |command| {
            match command {
                Command::Goto(target) if within_limits(&target) => {
                    target >> clear;
                    if clear.output {
                        target
                            >> retry
                            >> |(target, _attempt)| {
                                &mut arm >> motion.resources.0;
                                target >> motion;
                                motion
                            };
                        // Reads the arm after the motion; the edge orders the two uses.
                        &mut arm >> report.resources.0;
                        retry >> report;
                    } else {
                        target >> blocked;
                    }
                }
                Command::Goto(target) => {
                    target >> out_of_reach;
                }
                Command::Dwell(time) => {
                    time >> dwell;
                }
            }
        };

    refresh >> {
        panel;
    };

    Ok((
        arm,
        Outcome {
            attempts: retry.attempts(),
            out_of_reach: out_of_reach.targets.clone(),
            blocked: blocked.targets.clone(),
            panel: panel.shown.clone(),
        },
    ))
}

/// Run the task server with an operator console on its own thread, outside the
/// async runtime, so it sends with `fire_blocking`. A node or another task would
/// use `try_fire` or `fire_async().await`.
pub async fn run_console(
    arm: SimArm,
    script: Vec<Command>,
    keep_out: Range<f64>,
) -> RoplatResult<(SimArm, Outcome)> {
    let (orders, commands) = create_event_channel(8);
    let (refresh_button, refresh) = create_signal();
    let console = std::thread::spawn(move || {
        for command in script {
            orders
                .fire_blocking(command)
                .expect("the task server takes commands until the console stops");
            refresh_button.fire();
        }
        // Dropping both triggers lets their rhythms finish.
    });
    let served = serve(arm, commands, refresh, keep_out).await;
    console.join().expect("the console thread does not panic");
    served
}
