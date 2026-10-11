//! Conveyor tracking: two devices, two paces, one System. The belt ticks on its
//! own clock and publishes where the part is; the arm runs its own control
//! session and servoes toward the latest report. No graph edge joins them, so
//! neither waits for the other's tick.

use robot_behavior::JointPositionControl;
use robot_behavior::roplat::AsyncControlRhythm;
use roplat::RoplatResult;
use roplat::comm::Latest;
use roplat::rhythm::WithPeriod;

use crate::conveyor::{BeltSpec, Conveyor, PartReport};
use crate::nodes::{JointServo, PartBroadcast, PartTracker};
use crate::sim::{DOF, SimArm};

type ArmControl = AsyncControlRhythm<SimArm, JointPositionControl<DOF>>;

#[roplat::system]
pub async fn track_part(arm: SimArm, belt: BeltSpec) -> RoplatResult<SimArm> {
    let part = Latest::<PartReport>::new(1);

    let mut belt_clock = (0..belt.ticks).with_period(belt.period);
    let mut conveyor = Conveyor::new(belt);
    let mut broadcast = PartBroadcast::new(part.publisher());

    let mut control = ArmControl::new();
    let mut tracker = PartTracker::new(part.subscriber(0), belt.offset);
    let mut servo = JointServo::default();

    belt_clock >> {
        conveyor >> broadcast;
    };

    &mut arm >> control >> |(state, _dt)| {
        state >> tracker;
        state >> servo.input.0;
        tracker >> servo.input.1;
        servo
    };
    control >> &mut arm;

    Ok(arm.into_inner())
}
