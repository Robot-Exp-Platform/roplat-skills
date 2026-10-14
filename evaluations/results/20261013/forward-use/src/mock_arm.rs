//! Device-free, immediate-completion joint-motion backend. No transport or timer.
use std::sync::{Arc, Mutex};

use robot_behavior::driver::*;
use robot_behavior::{RobotException, RobotResult};
use roplat::Lifecycle;

pub type Goal = [f64; 2];

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Connected(&'static str),
    Accepted(&'static str, Goal),
    Rejected(&'static str, Goal),
    Disconnected(&'static str),
}

/// Mock backend event sink, inspected by the caller after the graph returns.
#[derive(Clone, Default)]
pub struct Journal(Arc<Mutex<Vec<Event>>>);

impl Journal {
    fn record(&self, event: Event) {
        self.0.lock().expect("mock journal poisoned").push(event);
    }

    pub fn events(&self) -> Vec<Event> {
        self.0.lock().expect("mock journal poisoned").clone()
    }
}

pub struct BenchArm {
    name: &'static str,
    joints: Goal,
    connected: bool,
    journal: Journal,
}

impl BenchArm {
    pub fn new(name: &'static str, joints: Goal, journal: Journal) -> Self {
        Self {
            name,
            joints,
            connected: false,
            journal,
        }
    }

    pub fn joints(&self) -> Goal {
        self.joints
    }
    pub fn connected(&self) -> bool {
        self.connected
    }
}

impl Robot for BenchArm {
    type State = JointState<2>;
    const CONTROL_PERIOD: f64 = 0.002;

    fn version() -> String {
        "device-free-bench-arm".into()
    }

    fn read_state(&mut self) -> RobotResult<Self::State> {
        if !self.connected {
            return Err(RobotException::CommandException(
                "arm is disconnected".into(),
            ));
        }
        Ok(JointState::from_meas(JointSample {
            q: Some(self.joints),
            ..Default::default()
        }))
    }

    fn init(&mut self) -> RobotResult<()> {
        if self.connected {
            return Err(RobotException::ConflictingInstruction(
                "arm already connected".into(),
            ));
        }
        self.connected = true;
        self.journal.record(Event::Connected(self.name));
        Ok(())
    }

    fn shutdown(&mut self) -> RobotResult<()> {
        if !self.connected {
            return Err(RobotException::ConflictingInstruction(
                "arm already disconnected".into(),
            ));
        }
        self.connected = false;
        self.journal.record(Event::Disconnected(self.name));
        Ok(())
    }
}

impl Joints<2> for BenchArm {
    const JOINT_MIN: Goal = [-1.0; 2];
    const JOINT_MAX: Goal = [1.0; 2];
}

impl MoveTo<JointSpace<2>> for BenchArm {
    fn move_to(&mut self, target: Goal) -> RobotResult<()> {
        if !self.connected {
            return Err(RobotException::CommandException(
                "arm is disconnected".into(),
            ));
        }
        if target
            .iter()
            .any(|q| !q.is_finite() || !(-1.0..=1.0).contains(q))
        {
            self.journal.record(Event::Rejected(self.name, target));
            return Err(RobotException::InvalidInstruction(format!(
                "{} target exceeds mock joint limits",
                self.name
            )));
        }
        self.joints = target;
        self.journal.record(Event::Accepted(self.name, target));
        Ok(())
    }
}

// Explicitly adapt Robot lifecycle; the two traits are not automatically linked.
impl Lifecycle for BenchArm {
    type Error = RobotException;
    async fn on_init(&mut self) -> RobotResult<()> {
        Robot::init(self)
    }
    async fn on_shutdown(&mut self) -> RobotResult<()> {
        Robot::shutdown(self)
    }
}
