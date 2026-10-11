//! A simulated 6-axis arm exposing the same capability traits as a real driver.
//! Swap `SimArm` for a real driver type; the application files stay unchanged.

use std::{ops::ControlFlow, time::Duration};

use robot_behavior::{
    AsyncControlCallback, AsyncControlWith, JointPositionControl, JointSample, JointState, Robot,
    RobotResult,
};

pub const DOF: usize = 6;
pub type Joints = [f64; DOF];

pub struct SimArm {
    q: Joints,
    period: Duration,
}

impl SimArm {
    pub fn new(q: Joints) -> Self {
        Self { q, period: Duration::from_secs_f64(Self::CONTROL_PERIOD) }
    }

    pub fn joints(&self) -> Joints {
        self.q
    }

    fn observe(&self) -> JointState<DOF> {
        JointState::from_meas(JointSample { q: Some(self.q), ..Default::default() })
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
                    // Stand-in for the vendor's low-level joint position servo.
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
