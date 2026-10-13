//! A simulated 6-axis arm with the same capability traits as a real driver.
//! Swap `SimArm` for a real driver type; the application files stay unchanged.

use std::{ops::ControlFlow, time::Duration};

use robot_behavior::{
    AsyncControlCallback, AsyncControlWith, JointPositionControl, JointSample, JointSpace,
    JointState, Joints as JointLimits, MoveTo, Robot, RobotResult,
};
use roplat::{Lifecycle, RoplatError, RoplatResult};

pub const DOF: usize = 6;
pub type Joints = [f64; DOF];

pub struct SimArm {
    q: Joints,
    period: Duration,
    connected: bool,
}

impl SimArm {
    pub fn new(q: Joints) -> Self {
        Self {
            q,
            period: Duration::from_secs_f64(Self::CONTROL_PERIOD),
            connected: false,
        }
    }

    pub fn joints(&self) -> Joints {
        self.q
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    fn observe(&self) -> JointState<DOF> {
        JointState::from_meas(JointSample {
            q: Some(self.q),
            ..Default::default()
        })
    }
}

/// Connect/disconnect. A graph that creates the arm enables and closes it; an
/// arm passed in is managed by the caller.
impl Lifecycle for SimArm {
    type Error = RoplatError;

    async fn on_init(&mut self) -> RoplatResult<()> {
        self.connected = true;
        Ok(())
    }

    async fn on_shutdown(&mut self) -> RoplatResult<()> {
        self.connected = false;
        Ok(())
    }
}

impl Robot for SimArm {
    type State = JointState<DOF>;
    const CONTROL_PERIOD: f64 = 0.002;

    fn version() -> String {
        "sim-arm".into()
    }

    fn read_state(&mut self) -> RobotResult<Self::State> {
        Ok(self.observe())
    }
}

impl JointLimits<DOF> for SimArm {
    const JOINT_MIN: Joints = [-3.0; DOF];
    const JOINT_MAX: Joints = [3.0; DOF];
}

impl MoveTo<JointSpace<DOF>> for SimArm {
    /// Stand-in for a vendor's blocking point-to-point motion.
    fn move_to(&mut self, target: Joints) -> RobotResult<()> {
        self.q = target;
        Ok(())
    }
}

impl AsyncControlWith<JointPositionControl<DOF>> for SimArm {
    async fn control_native_async<C>(&mut self, callback: &mut C) -> RobotResult<()>
    where
        C: AsyncControlCallback<JointState<DOF>, Joints>,
    {
        let mut ticker = tokio::time::interval(self.period);
        loop {
            ticker.tick().await;
            match callback.call(self.observe(), self.period).await {
                ControlFlow::Continue((command, done)) => {
                    // Stand-in for the vendor's low-level joint servo.
                    for (q, target) in self.q.iter_mut().zip(command) {
                        *q += 0.5 * (target - *q);
                    }
                    if done {
                        return Ok(());
                    }
                }
                ControlFlow::Break(()) => return Ok(()),
            }
        }
    }
}
