use daiw_audio::{EffectiveNote, Instrument, Origin, RenderRequest, Snapshot, Track};

pub fn request() -> RenderRequest {
    RenderRequest::new(Snapshot {
        revision: "numeric".into(),
        tempo: 120.0,
        tempo_map: None,
        length: 1.0,
        tracks: vec![Track {
            id: "tone".into(),
            name: "Tone".into(),
            role: "music".into(),
            color: "#fff".into(),
            instrument: Instrument::Synth,
            gain: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            cutoff: 20_000.0,
        }],
        notes: vec![EffectiveNote {
            id: "note".into(),
            start: 0.0,
            duration: 1.0,
            pitch: 60.25,
            velocity: 0.8,
            instance_id: "instance".into(),
            track_id: "tone".into(),
            origin: Origin {
                definition_id: "definition".into(),
                note_id: "source".into(),
                modifier_ids: vec![],
            },
        }],
        regions: vec![],
    })
}
