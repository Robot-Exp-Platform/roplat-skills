//! A simulated conveyor: a part rides a straight belt at constant speed. The
//! belt has its own clock; one `process` call is one belt tick.

use std::time::Duration;

use roplat::{Lifecycle, Node, RoplatError};

/// Geometry and pace of the belt, in the arm's base frame. The belt runs along
/// +y at a fixed distance `offset` in front of the arm.
#[derive(Clone, Copy, Debug)]
pub struct BeltSpec {
    pub offset: f64,
    pub entry: f64,
    pub exit: f64,
    pub speed: f64,
    pub period: Duration,
    pub ticks: u32,
}

/// What the belt tells the rest of the cell each tick.
#[derive(Clone, Copy, Debug)]
pub struct PartReport {
    pub position: f64,
    pub left: bool,
}

pub struct Conveyor {
    position: f64,
    exit: f64,
    step: f64,
}

impl Conveyor {
    pub fn new(spec: BeltSpec) -> Self {
        Self {
            position: spec.entry,
            exit: spec.exit,
            step: spec.speed * spec.period.as_secs_f64(),
        }
    }
}

impl Lifecycle for Conveyor {
    type Error = RoplatError;
}

impl Node for Conveyor {
    type Input = ();
    type Output = PartReport;

    async fn process(&mut self, (): ()) -> PartReport {
        // The part stops being reachable at the exit; report it as gone there.
        self.position = (self.position + self.step).min(self.exit);
        PartReport { position: self.position, left: self.position >= self.exit }
    }
}
