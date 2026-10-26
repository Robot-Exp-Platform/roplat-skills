//! Finite 0.9 source example: a simulated planar arm and pure-data algorithms.
//! No hardware, model file, renderer or simulator process is opened.

use roplat::RoplatResult;
use roplat::rhythm::IntoRhythm;
use roplat_behavior::roplat::MotionNode;
use roplat_behavior::{
    ForwardKinematics, JointSample, JointSpace, JointState, ModelFormat, ModelSource, MoveTo, Pose,
    Robot, RobotDescription, RobotResult, SimulationModel, SpatialSample, select_simulation_model,
};
use roplat_control::joint_pd_control;
use roplat_kinematics::{ArmForwardKinematics, DhParam};
use roplat_trajectory::path_generate::constant_joint_target_fn;
use std::time::Duration;

struct PlanarArm {
    q: [f64; 2],
}

impl Robot for PlanarArm {
    type State = JointState<2>;
    const CONTROL_PERIOD: f64 = 0.002;
    fn version() -> String {
        "skill-simulated-arm".into()
    }
    fn read_state(&mut self) -> RobotResult<Self::State> {
        Ok(JointState::from_meas(
            JointSample::EMPTY.with_q(self.q).with_dq([0.; 2]),
        ))
    }
}

impl roplat_behavior::Joints<2> for PlanarArm {
    const JOINT_MIN: [f64; 2] = [-3.; 2];
    const JOINT_MAX: [f64; 2] = [3.; 2];
}

impl MoveTo<JointSpace<2>> for PlanarArm {
    fn move_to(&mut self, goal: [f64; 2]) -> RobotResult<()> {
        self.q = goal;
        Ok(())
    }
}

impl ArmForwardKinematics<2> for PlanarArm {
    const DH: [DhParam; 2] = [
        DhParam::DH {
            theta: 0.,
            d: 0.,
            r: 1.,
            alpha: 0.,
        },
        DhParam::DH {
            theta: 0.,
            d: 0.,
            r: 1.,
            alpha: 0.,
        },
    ];
}
roplat_kinematics::dh_kinematics!(PlanarArm, 2);

impl RobotDescription for PlanarArm {
    const SIMULATION_MODELS: &'static [SimulationModel] = &[
        SimulationModel::new(ModelFormat::Urdf, "planar.urdf"),
        SimulationModel::new(ModelFormat::Mjcf, "planar.xml").for_backend("mujoco"),
    ];
}

#[roplat::system]
async fn visit(mut arm: PlanarArm, goals: Vec<[f64; 2]>) -> RoplatResult<PlanarArm> {
    let mut tasks = goals.into_iter().into_rhythm();
    let mut motion = MotionNode::<PlanarArm, JointSpace<2>>::new();
    tasks
        >> |goal| {
            &mut arm >> motion.resources.0;
            goal >> motion;
            motion?;
        };
    Ok(arm)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arm = PlanarArm { q: [0.; 2] };
    let end = arm.forward_kinematics([0.; 2])?;
    assert!((end.position()[0] - 2.).abs() < 1e-12);

    let pose = Pose::from_euler([1., 2., 3.], [0.; 3]);
    let values = Vec::<f64>::from(pose);
    assert_eq!(values, vec![1., 2., 3., 0., 0., 0., 1.]);
    let spatial = SpatialSample::EMPTY
        .with_pose(pose)
        .with_twist([0.; 6].into());
    assert_eq!(spatial.pose().unwrap().position(), [1., 2., 3.]);
    assert!(spatial.accel().is_none());
    assert!(SpatialSample::default().accel().is_some());

    let mut sample = JointSample::<2>::EMPTY.with_q([0.; 2]).with_dq([0.; 2]);
    assert_eq!(sample.q(), Some([0.; 2]));
    sample.set_q(None);
    assert!(sample.q().is_none());
    sample.set_q(Some([0.; 2]));
    let state = JointState::from_meas(sample);
    let mut target = constant_joint_target_fn([0.5; 2]);
    assert_eq!(target(state, Duration::from_millis(2)), ([0.5; 2], false));
    let mut control = joint_pd_control([0.5; 2], [2.; 2], [0.1; 2]);
    assert_eq!(control(state, Duration::from_millis(2)), ([1.; 2], false));

    let selected = select_simulation_model::<PlanarArm>("mujoco", &[ModelFormat::Mjcf])?;
    assert_eq!(selected.model.path, "planar.xml");
    assert_eq!(selected.source, ModelSource::BackendSpecific);
    assert!(select_simulation_model::<PlanarArm>("unknown", &[ModelFormat::Sdf]).is_err());

    let mut arm = visit(arm, vec![[0.2; 2], [0.5; 2]]).await?;
    assert_eq!(arm.read_state()?.meas.q(), Some([0.5; 2]));
    println!("0.9: pose, validity, split algorithms, model selection and resource graph passed");
    Ok(())
}
