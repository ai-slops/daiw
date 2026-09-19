use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

pub const TICKS_PER_BEAT: i64 = 960;
pub const MAX_BEATS: f64 = 1_000_000.0;
pub const MAX_TICKS: i64 = 960_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Tick(i64);

impl Tick {
    pub fn from_beats(beats: f64) -> Result<Self, &'static str> {
        if !beats.is_finite() {
            return Err("beat value must be finite");
        }
        if beats.abs() > MAX_BEATS {
            return Err("beat value exceeds timeline limit");
        }
        let ticks = beats * TICKS_PER_BEAT as f64;
        let rounded = ticks.round();
        if (ticks - rounded).abs() > 1.0e-7
            || rounded > i64::MAX as f64
            || rounded < i64::MIN as f64
        {
            return Err("beat value must be quantized to 1/960 beat");
        }
        Ok(Self(rounded as i64))
    }

    #[must_use]
    pub const fn from_ticks(ticks: i64) -> Self {
        Self(ticks)
    }

    #[must_use]
    pub const fn ticks(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn beats(self) -> f64 {
        self.0 as f64 / TICKS_PER_BEAT as f64
    }
}

impl std::ops::Add for Tick {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Serialize for Tick {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.beats())
    }
}

impl<'de> Deserialize<'de> for Tick {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let beats = f64::deserialize(deserializer)?;
        Self::from_beats(beats).map_err(de::Error::custom)
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.beats())
    }
}
