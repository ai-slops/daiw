use crate::music::Instrument;

pub(super) struct FixtureIdentity<'a> {
    pub id: &'a str,
    pub name: &'a str,
}
impl<'a> FixtureIdentity<'a> {
    pub const fn new(id: &'a str, name: &'a str) -> Self {
        Self { id, name }
    }
}

pub(super) struct FixtureTrackStyle<'a> {
    pub role: &'a str,
    pub color: &'a str,
    pub instrument: Instrument,
}
impl<'a> FixtureTrackStyle<'a> {
    pub const fn new(role: &'a str, color: &'a str, instrument: Instrument) -> Self {
        Self {
            role,
            color,
            instrument,
        }
    }
}

pub(super) struct FixtureRoute<'a> {
    pub definition: &'a str,
    pub track: &'a str,
}
impl<'a> FixtureRoute<'a> {
    pub const fn new(definition: &'a str, track: &'a str) -> Self {
        Self { definition, track }
    }
}

pub(super) struct FixtureTiming {
    pub start: f64,
    pub length: f64,
}
impl FixtureTiming {
    pub const fn new(start: f64, length: f64) -> Self {
        Self { start, length }
    }
}

pub(super) struct FixtureExpression {
    pub pitch: f64,
    pub velocity: f64,
}
impl FixtureExpression {
    pub const fn new(pitch: f64, velocity: f64) -> Self {
        Self { pitch, velocity }
    }
}
