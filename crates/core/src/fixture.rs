use crate::fixture_values::{
    FixtureExpression, FixtureIdentity, FixtureRoute, FixtureTiming, FixtureTrackStyle,
};
use crate::ids::{DefinitionId, InstanceId, NoteId, ProjectId, Revision, TrackId};
use crate::music::{Definition, Instance, Instrument, Note, Track};
use crate::project::Project;
use crate::time::Tick;
use std::collections::BTreeMap;

#[must_use]
pub fn create_project(name: Option<&str>, fixture: bool) -> Project {
    let mut project = Project {
        id: ProjectId::new_random(),
        name: name.unwrap_or("Shared Chorus Study").to_owned(),
        revision: Revision::initial(),
        tempo: 112.0,
        meter: [4, 4],
        tempo_map: None,
        meter_map: None,
        definitions: Vec::new(),
        instances: Vec::new(),
        tracks: Vec::new(),
        modifiers: Vec::new(),
        proposals: Vec::new(),
        history: Vec::new(),
        regions: Vec::new(),
        assets: Vec::new(),
    };
    if fixture {
        add_fixture(&mut project);
    }
    project
}

fn add_fixture(project: &mut Project) {
    project.tracks = vec![
        track(
            FixtureIdentity::new("track-drums", "Neon Drums"),
            FixtureTrackStyle::new("rhythm", "#fb7185", Instrument::Kick),
        ),
        track(
            FixtureIdentity::new("track-bass", "Sub Bass"),
            FixtureTrackStyle::new("bass", "#f59e0b", Instrument::Bass),
        ),
        track(
            FixtureIdentity::new("track-pad", "Glass Pad"),
            FixtureTrackStyle::new("harmony", "#22d3ee", Instrument::Pad),
        ),
        track(
            FixtureIdentity::new("track-lead", "Aurora Lead"),
            FixtureTrackStyle::new("melody", "#a78bfa", Instrument::Synth),
        ),
        track(
            FixtureIdentity::new("track-fx", "Night Air"),
            FixtureTrackStyle::new("texture", "#34d399", Instrument::Sample),
        ),
    ];
    project.definitions = vec![
        chorus(),
        definition(
            FixtureIdentity::new("def-drums", "Four on the Floor"),
            8.0,
            &[(0.0, 36.0), (2.0, 36.0), (4.0, 38.0), (6.0, 36.0)],
        ),
        definition(
            FixtureIdentity::new("def-bass", "Rolling Bass"),
            8.0,
            &[(0.0, 36.0), (2.0, 36.0), (4.0, 43.0), (6.0, 34.0)],
        ),
        definition(
            FixtureIdentity::new("def-pad", "Wide Pad"),
            16.0,
            &[(0.0, 48.0), (0.0, 55.0), (8.0, 53.0)],
        ),
    ];
    project.instances = vec![
        instance(
            FixtureIdentity::new("instance-chorus-a", "Chorus I"),
            FixtureRoute::new("def-chorus", "track-lead"),
            FixtureTiming::new(24.0, 8.0),
        ),
        instance(
            FixtureIdentity::new("instance-chorus-b", "Chorus II"),
            FixtureRoute::new("def-chorus", "track-lead"),
            FixtureTiming::new(56.0, 8.0),
        ),
        instance(
            FixtureIdentity::new("instance-chorus-c", "Final Chorus"),
            FixtureRoute::new("def-chorus", "track-lead"),
            FixtureTiming::new(120.0, 8.0),
        ),
        instance(
            FixtureIdentity::new("instance-drums", "Drum Bed"),
            FixtureRoute::new("def-drums", "track-drums"),
            FixtureTiming::new(0.0, 128.0),
        ),
        instance(
            FixtureIdentity::new("instance-bass", "Bass Bed"),
            FixtureRoute::new("def-bass", "track-bass"),
            FixtureTiming::new(0.0, 128.0),
        ),
        instance(
            FixtureIdentity::new("instance-pad", "Pad Bed"),
            FixtureRoute::new("def-pad", "track-pad"),
            FixtureTiming::new(0.0, 128.0),
        ),
    ];
}

fn tick(beats: f64) -> Tick {
    Tick::from_ticks((beats * 960.0).round() as i64)
}

fn track(identity: FixtureIdentity<'_>, style: FixtureTrackStyle<'_>) -> Track {
    Track {
        id: TrackId::from(identity.id),
        name: identity.name.to_owned(),
        role: style.role.to_owned(),
        color: style.color.to_owned(),
        instrument: style.instrument,
        gain: 0.0,
        pan: 0.0,
        mute: false,
        solo: false,
        cutoff: 12_000.0,
    }
}

fn chorus() -> Definition {
    Definition {
        id: DefinitionId::from("def-chorus"),
        name: "Linked Aurora Chorus".to_owned(),
        length: tick(8.0),
        notes: vec![
            note(
                "note-chorus-root",
                FixtureTiming::new(0.0, 2.0),
                FixtureExpression::new(61.0, 0.86),
            ),
            note(
                "note-chorus-two",
                FixtureTiming::new(2.0, 2.0),
                FixtureExpression::new(64.0, 0.80),
            ),
            note(
                "note-chorus-three",
                FixtureTiming::new(4.0, 2.0),
                FixtureExpression::new(68.0, 0.84),
            ),
            note(
                "note-chorus-four",
                FixtureTiming::new(6.0, 2.0),
                FixtureExpression::new(66.0, 0.82),
            ),
        ],
        revision: 0,
        slots: BTreeMap::from([(
            "ending-fill".to_owned(),
            crate::music::NoteSlot {
                start: tick(6.0),
                end: tick(8.0),
            },
        )]),
    }
}

fn definition(identity: FixtureIdentity<'_>, length: f64, pitches: &[(f64, f64)]) -> Definition {
    Definition {
        id: DefinitionId::from(identity.id),
        name: identity.name.to_owned(),
        length: tick(length),
        notes: pitches
            .iter()
            .enumerate()
            .map(|(index, (start, pitch))| {
                note(
                    &format!("{}-{index}", identity.id),
                    FixtureTiming::new(*start, 0.5),
                    FixtureExpression::new(*pitch, 0.78),
                )
            })
            .collect(),
        revision: 0,
        slots: BTreeMap::new(),
    }
}

fn note(id: &str, timing: FixtureTiming, expression: FixtureExpression) -> Note {
    Note {
        id: NoteId::from(id),
        start: tick(timing.start),
        duration: tick(timing.length),
        pitch: expression.pitch,
        velocity: expression.velocity,
    }
}

fn instance(
    identity: FixtureIdentity<'_>,
    route: FixtureRoute<'_>,
    timing: FixtureTiming,
) -> Instance {
    Instance {
        id: InstanceId::from(identity.id),
        name: identity.name.to_owned(),
        definition_id: DefinitionId::from(route.definition),
        track_id: TrackId::from(route.track),
        start: tick(timing.start),
        length: tick(timing.length),
        modifiers: Vec::new(),
        overrides: Vec::new(),
        modifier_bindings: BTreeMap::new(),
        modifier_ordering: None,
    }
}
