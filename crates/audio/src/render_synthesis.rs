use std::f64::consts::TAU;

use crate::Instrument;
use crate::signal_graph::PreparedNote;

pub(crate) fn accumulate_note(
    frames: &mut [[f32; 2]],
    block_start: usize,
    note: &PreparedNote,
    sample_rate: u32,
) {
    let start = note.start.max(block_start);
    let end = note.end.min(block_start + frames.len());
    if start >= end {
        return;
    }
    for absolute_frame in start..end {
        let offset = absolute_frame - note.start;
        let time = offset as f64 / f64::from(sample_rate);
        let progress = offset as f64 / note.frames.max(1) as f64;
        let envelope = note_envelope(progress, note.instrument);
        let wave = instrument_sample(note.instrument, note.frequency, time, offset as u64);
        let sample = (wave * envelope * note.velocity.clamp(0.0, 1.0)) as f32;
        let output = &mut frames[absolute_frame - block_start];
        output[0] += sample;
        output[1] += sample;
    }
}

fn note_envelope(progress: f64, instrument: Instrument) -> f64 {
    let attack = match instrument {
        Instrument::Pad => 0.2,
        Instrument::Synth
        | Instrument::Bass
        | Instrument::Kick
        | Instrument::Snare
        | Instrument::Hat
        | Instrument::Sample => 0.01,
    };
    let onset = (progress / attack).min(1.0);
    let release = ((1.0 - progress) / 0.12).clamp(0.0, 1.0);
    onset * release
}

fn instrument_sample(instrument: Instrument, frequency: f64, time: f64, seed: u64) -> f64 {
    let phase = (frequency * time).fract();
    match instrument {
        Instrument::Synth => (phase * 2.0 - 1.0) * 0.25,
        Instrument::Bass => ((TAU * phase).sin() + 0.35 * (TAU * phase * 2.0).sin()) * 0.3,
        Instrument::Kick => {
            (TAU * (45.0 + 95.0 * (-time * 18.0).exp()) * time).sin() * (-time * 10.0).exp() * 0.9
        }
        Instrument::Snare => noise(seed) * (-time * 14.0).exp() * 0.5,
        Instrument::Hat => noise(seed.wrapping_mul(17)) * (-time * 45.0).exp() * 0.28,
        Instrument::Pad => ((TAU * phase).sin() + 0.5 * (TAU * (phase + time * 0.7)).sin()) * 0.18,
        Instrument::Sample => 0.0,
    }
}

fn noise(seed: u64) -> f64 {
    let value = seed
        .wrapping_add(0x9e37_79b9_7f4a_7c15)
        .wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let mixed = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((mixed ^ (mixed >> 31)) as f64 / u64::MAX as f64) * 2.0 - 1.0
}
