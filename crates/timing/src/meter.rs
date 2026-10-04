use crate::{MAX_TICKS, TICKS_PER_BEAT, Tick, TimingError};
use serde::{Deserialize, Deserializer, Serialize, de};

const MAX_POINTS: usize = 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeterPoint {
    pub at: Tick,
    pub numerator: u8,
    pub denominator: u8,
}

impl MeterPoint {
    #[must_use]
    pub const fn new(at: Tick, numerator: u8, denominator: u8) -> Self {
        Self {
            at,
            numerator,
            denominator,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MeterMap {
    points: Vec<MeterPoint>,
}

impl MeterMap {
    pub fn new(points: Vec<MeterPoint>) -> Result<Self, TimingError> {
        if points.is_empty() || points.len() > MAX_POINTS {
            return Err(TimingError::PointCount);
        }
        if points.first().is_none_or(|point| point.at.ticks() != 0) {
            return Err(TimingError::Start);
        }
        if points
            .iter()
            .any(|point| !(0..=MAX_TICKS).contains(&point.at.ticks()))
        {
            return Err(TimingError::BeatRange);
        }
        if points.windows(2).any(|pair| pair[0].at >= pair[1].at) {
            return Err(TimingError::Order);
        }
        if points
            .iter()
            .any(|point| point.numerator == 0 || point.denominator == 0)
        {
            return Err(TimingError::Meter);
        }
        for pair in points.windows(2) {
            let delta = i128::from(pair[1].at.ticks() - pair[0].at.ticks());
            let scaled_delta = delta * i128::from(pair[0].denominator);
            let bar_units = i128::from(pair[0].numerator) * 4 * i128::from(TICKS_PER_BEAT);
            if scaled_delta % bar_units != 0 {
                return Err(TimingError::MeterBoundary);
            }
        }
        Ok(Self { points })
    }

    #[must_use]
    pub fn points(&self) -> &[MeterPoint] {
        &self.points
    }
}

impl<'de> Deserialize<'de> for MeterMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            points: Vec<MeterPoint>,
        }
        Self::new(Wire::deserialize(deserializer)?.points).map_err(de::Error::custom)
    }
}
