//! A heavy planner beside a live control loop. Planning holds its thread for
//! tens of milliseconds, as an optimiser or a long foreign call does; the arm
//! must keep its 2 ms pace meanwhile. The planner therefore runs in a hard
//! rhythm, entered by an owned domain `move |…| { … }`: every solve happens on
//! the rhythm's own thread, and the planner object travels there and back with
//! each drive. The two domains meet only through a latest-value sideband.

use std::time::{Duration, Instant};

use roplat::comm::Latest;
use roplat::rhythm::{IntoHard, IntoRhythm, once};
use roplat::{Lifecycle, Node, RoplatError, RoplatResult};
use roplat_behavior::JointPositionControl;
use roplat_behavior::roplat::AsyncControlRhythm;

use crate::nodes::{JointServo, Mirror, Reference, Share};
use crate::sim::{DOF, Joints, SimArm};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

/// Stand-in for a motion planner. Its solve is synchronous and blocks the
/// thread it runs on; the numerics are a placeholder.
pub struct Planner {
    solve_time: Duration,
}

impl Planner {
    pub fn new(solve_time: Duration) -> Self {
        Self { solve_time }
    }
}

impl Lifecycle for Planner {
    type Error = RoplatError;
}

impl Node for Planner {
    type Resources = ();
    /// A goal and whether it is the last one.
    type Input = (Joints, bool);
    type Output = Reference;

    async fn process(&mut self, (goal, last): (Joints, bool), _: ()) -> Reference {
        std::thread::sleep(self.solve_time);
        (goal, last)
    }
}

/// Control-loop watchdog: the longest time between two cycles, which a real
/// application would alarm on when it exceeds a few periods.
#[derive(Default)]
pub struct CycleWatch {
    last: Option<Instant>,
    pub longest_gap: Duration,
}

impl Lifecycle for CycleWatch {
    type Error = RoplatError;
}

impl Node for CycleWatch {
    type Resources = ();
    type Input = ();
    type Output = ();

    async fn process(&mut self, (): (), _: ()) {
        let now = Instant::now();
        if let Some(last) = self.last.replace(now) {
            self.longest_gap = self.longest_gap.max(now - last);
        }
    }
}

#[roplat::system]
pub async fn plan_while_controlling(
    mut arm: SimArm,
    goals: Vec<Joints>,
    solve_time: Duration,
) -> RoplatResult<(SimArm, Duration)> {
    let count = goals.len();
    let mut tasks = goals
        .into_iter()
        .enumerate()
        .map(move |(index, goal)| (goal, index + 1 == count))
        .into_rhythm();
    let mut plan = once::<(Joints, bool), Reference>().hard().name("planner");
    let mut planner = Planner::new(solve_time);
    let plans = Latest::<Reference>::new(1);
    let mut publish = Share::new(plans.publisher());

    let mut control = ArmControl::new();
    // Follows the newest plan; holds the arm's own position until the first.
    let mut follow = Mirror::new(plans.subscriber(0), [0.0; DOF]);
    let mut servo = JointServo::default();
    let mut watch = CycleWatch::default();

    // Planning domain: one solve per goal, on the planner's own thread.
    tasks
        >> |goal| {
            goal >> plan
                >> move |goal| {
                    goal >> planner;
                    planner
                };
            plan.output >> publish;
        };

    // Control domain: the arm's own rhythm, unaffected by the solves.
    &mut arm >> control.resources.0;
    control
        >> |(state, _dt)| {
            watch;
            state >> follow;
            state >> servo.input.0;
            follow >> servo.input.1;
            servo
        };

    Ok((arm, watch.longest_gap))
}
