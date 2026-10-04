use crate::{MAX_TICKS, TICKS_PER_BEAT, Tick};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const MIN_POINTS: usize = 1;
const MAX_POINTS: usize = 1024;
const MIN_BPM: f64 = 20.0;
const MAX_BPM: f64 = 400.0;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Curve {
    Step,
    Linear,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoPoint {
    pub at: Tick,
    pub bpm: f64,
    pub curve: Curve,
}

impl TempoPoint {
    #[must_use]
    pub const fn new(at: Tick, bpm: f64, curve: Curve) -> Self {
        Self { at, bpm, curve }
    }
}

#[derive(Clone, Debug)]
pub struct TempoMap {
    points: Vec<TempoPoint>,
    prefix_seconds: Vec<f64>,
}

impl PartialEq for TempoMap {
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TimingError {
    #[error("timing map must contain 1..=1024 points")]
    PointCount,
    #[error("timing map must start at beat 0")]
    Start,
    #[error("timing map points must be strictly increasing")]
    Order,
    #[error("beat must be within the supported timeline")]
    BeatRange,
    #[error("seconds must be finite, nonnegative, and resolve within the supported timeline")]
    SecondsRange,
    #[error("tempo must be finite and between 20 and 400 BPM")]
    Tempo,
    #[error("the final tempo point must use a step curve")]
    LastCurve,
    #[error("meter numerator and denominator must be positive")]
    Meter,
    #[error("meter changes must occur on a boundary of the previous meter")]
    MeterBoundary,
}

impl TempoMap {
    pub fn new(points: Vec<TempoPoint>) -> Result<Self, TimingError> {
        validate_points(&points)?;
        let mut prefix_seconds = Vec::with_capacity(points.len());
        prefix_seconds.push(0.0);
        for pair in points.windows(2) {
            let elapsed = segment_seconds(pair[0], pair[1], pair[1].at.ticks());
            let prefix = prefix_seconds.last().copied().unwrap_or(0.0);
            prefix_seconds.push(prefix + elapsed);
        }
        Ok(Self {
            points,
            prefix_seconds,
        })
    }

    #[must_use]
    pub fn points(&self) -> &[TempoPoint] {
        &self.points
    }

    pub fn seconds_at(&self, at: Tick) -> Result<f64, TimingError> {
        if !(0..=MAX_TICKS).contains(&at.ticks()) {
            return Err(TimingError::BeatRange);
        }
        let index = self.points.partition_point(|point| point.at <= at);
        let segment = index.saturating_sub(1).min(self.points.len() - 1);
        let point = self.points[segment];
        let prefix = self.prefix_seconds[segment];
        Ok(match self.points.get(segment + 1) {
            Some(next) => prefix + segment_seconds(point, *next, at.ticks()),
            None => prefix + step_seconds(point, at.ticks()),
        })
    }

    pub fn beat_at(&self, seconds: f64) -> Result<Tick, TimingError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(TimingError::SecondsRange);
        }
        let index = self
            .prefix_seconds
            .partition_point(|prefix| *prefix <= seconds);
        let segment = index.saturating_sub(1).min(self.points.len() - 1);
        let point = self.points[segment];
        let elapsed = seconds - self.prefix_seconds[segment];
        let beats = match self.points.get(segment + 1) {
            Some(next) if point.curve == Curve::Linear => {
                let span = (next.at.ticks() - point.at.ticks()) as f64 / TICKS_PER_BEAT as f64;
                let slope = (next.bpm - point.bpm) / span;
                if slope == 0.0 {
                    elapsed * point.bpm / 60.0
                } else {
                    point.bpm / slope * (elapsed * slope / 60.0).exp_m1()
                }
            }
            Some(_) | None => elapsed * point.bpm / 60.0,
        };
        let ticks = point.at.ticks() as f64 + beats * TICKS_PER_BEAT as f64;
        if !ticks.is_finite() || !(0.0..=MAX_TICKS as f64).contains(&ticks) {
            return Err(TimingError::SecondsRange);
        }
        Ok(Tick::from_ticks(ticks.round() as i64))
    }
}

fn validate_points(points: &[TempoPoint]) -> Result<(), TimingError> {
    if !(MIN_POINTS..=MAX_POINTS).contains(&points.len()) {
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
        .any(|point| !point.bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&point.bpm))
    {
        return Err(TimingError::Tempo);
    }
    if points.last().is_none_or(|point| point.curve != Curve::Step) {
        return Err(TimingError::LastCurve);
    }
    Ok(())
}

fn segment_seconds(point: TempoPoint, next: TempoPoint, at_ticks: i64) -> f64 {
    let elapsed_beats = (at_ticks - point.at.ticks()) as f64 / TICKS_PER_BEAT as f64;
    match point.curve {
        Curve::Step => 60.0 * elapsed_beats / point.bpm,
        Curve::Linear => {
            let span_beats = (next.at.ticks() - point.at.ticks()) as f64 / TICKS_PER_BEAT as f64;
            let slope = (next.bpm - point.bpm) / span_beats;
            if slope == 0.0 {
                60.0 * elapsed_beats / point.bpm
            } else {
                60.0 / slope * (slope * elapsed_beats / point.bpm).ln_1p()
            }
        }
    }
}

fn step_seconds(point: TempoPoint, at_ticks: i64) -> f64 {
    let elapsed_beats = (at_ticks - point.at.ticks()) as f64 / TICKS_PER_BEAT as f64;
    60.0 * elapsed_beats / point.bpm
}

impl Serialize for TempoMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wire<'a> {
            points: &'a [TempoPoint],
        }
        Wire {
            points: &self.points,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TempoMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            points: Vec<TempoPoint>,
        }
        Self::new(Wire::deserialize(deserializer)?.points).map_err(de::Error::custom)
    }
}
