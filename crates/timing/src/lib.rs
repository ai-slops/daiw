mod meter;
mod tempo;
mod tick;

pub use meter::{MeterMap, MeterPoint};
pub use tempo::{Curve, TempoMap, TempoPoint, TimingError};
pub use tick::{MAX_BEATS, MAX_TICKS, TICKS_PER_BEAT, Tick};

pub type Beat = Tick;
