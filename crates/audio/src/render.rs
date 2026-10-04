use std::f64::consts::TAU;

use crate::render_clock::{RenderClock, checked_frame_index, silent_buffer};
use crate::validate::{finite, finite_samples, positive, validate_request};
use crate::{AudioError, EffectiveNote, Instrument, PcmAsset, RenderRequest, RenderedAudio, Track};

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

pub fn render(request: &RenderRequest) -> Result<RenderedAudio, AudioError> {
    render_with_sample_rate(request, DEFAULT_SAMPLE_RATE)
}

pub fn render_with_sample_rate(
    request: &RenderRequest,
    sample_rate: u32,
) -> Result<RenderedAudio, AudioError> {
    validate_request(request, sample_rate)?;
    let clock = RenderClock::new(&request.snapshot, sample_rate)?;
    let samples = clock
        .total_frames
        .checked_mul(2)
        .ok_or(AudioError::InvalidFrameCount)?;
    let mut master = silent_buffer(samples)?;
    let mut stereo = Vec::new();
    let soloed = request.snapshot.tracks.iter().any(|track| track.solo);
    for track in &request.snapshot.tracks {
        if clock.total_frames == 0 || track.mute || (soloed && !track.solo) {
            continue;
        }
        if stereo.len() != samples {
            stereo = silent_buffer(samples)?;
        } else {
            stereo.fill(0.0);
        }
        for note in request
            .snapshot
            .notes
            .iter()
            .filter(|note| note.track_id == track.id)
        {
            render_note(&mut stereo, note, track.instrument, &clock)?;
        }
        for region in request
            .snapshot
            .regions
            .iter()
            .filter(|region| region.track_id == track.id)
        {
            let asset =
                request
                    .assets
                    .get(&region.asset_id)
                    .ok_or_else(|| AudioError::MissingAsset {
                        asset_id: region.asset_id.clone(),
                    })?;
            render_region(&mut stereo, region, asset, &clock)?;
        }
        lowpass(&mut stereo, track.cutoff, sample_rate);
        finite_samples(&stereo, &track.id)?;
        mix_track(&mut master, &stereo, track)?;
    }
    finite_samples(&master, "master")?;
    Ok(RenderedAudio {
        sample_rate,
        channels: 2,
        samples: master,
    })
}

fn render_note(
    output: &mut [f32],
    note: &EffectiveNote,
    instrument: Instrument,
    clock: &RenderClock,
) -> Result<(), AudioError> {
    if instrument == Instrument::Sample {
        return Ok(());
    }
    let start = clock.frame_at(note.start)?;
    if start >= clock.total_frames {
        return Ok(());
    }
    let frames = clock.frames_for(note.start, note.duration)?;
    let actual_frames = frames.min(clock.total_frames - start);
    if actual_frames == 0 {
        return Ok(());
    }
    let frequency = match instrument {
        Instrument::Synth | Instrument::Bass | Instrument::Pad => {
            let frequency = 440.0 * 2.0_f64.powf((note.pitch - 69.0) / 12.0);
            positive(&note.id, "frequency", frequency)?;
            let last_time = (actual_frames - 1) as f64 / f64::from(clock.sample_rate);
            finite(&note.id, "phase", frequency * last_time)?;
            frequency
        }
        Instrument::Kick | Instrument::Snare | Instrument::Hat | Instrument::Sample => 0.0,
    };
    for offset in 0..actual_frames {
        let time = offset as f64 / f64::from(clock.sample_rate);
        let progress = offset as f64 / frames.max(1) as f64;
        let envelope = note_envelope(progress, instrument);
        let wave = instrument_sample(instrument, frequency, time, offset as u64);
        let sample = (wave * envelope * note.velocity.clamp(0.0, 1.0)) as f32;
        let target = (start + offset) * 2;
        output[target] += sample;
        output[target + 1] += sample;
    }
    Ok(())
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

fn render_region(
    output: &mut [f32],
    region: &crate::AudioRegion,
    asset: &PcmAsset,
    clock: &RenderClock,
) -> Result<(), AudioError> {
    let start = clock.frame_at(region.start)?;
    if start >= clock.total_frames {
        return Ok(());
    }
    let duration = clock
        .frames_for(region.start, region.duration)?
        .min(clock.total_frames - start);
    if duration == 0 {
        return Ok(());
    }
    let source_start = checked_frame_index(
        (region.source_offset.max(0.0) * f64::from(asset.sample_rate)).round(),
    )?;
    let channel_count = usize::from(asset.channels);
    let source_frames = asset.samples.len() / channel_count;
    if source_start >= source_frames {
        return Ok(());
    }
    let remaining =
        u64::try_from(source_frames - source_start).map_err(|_| AudioError::InvalidFrameCount)?;
    for target_offset in 0..duration {
        let offset = u64::try_from(target_offset).map_err(|_| AudioError::InvalidFrameCount)?;
        let source_offset = offset
            .checked_mul(u64::from(asset.sample_rate))
            .ok_or(AudioError::InvalidFrameCount)?
            / u64::from(clock.sample_rate);
        if source_offset >= remaining {
            break;
        }
        let source_frame = source_start
            + usize::try_from(source_offset).map_err(|_| AudioError::InvalidFrameCount)?;
        let source = source_frame * channel_count;
        let frame = asset.samples.get(source..source + channel_count);
        if let Some(channels) = frame {
            let gain = 10.0_f32.powf(region.gain as f32 / 20.0);
            let target_sample = (start + target_offset) * 2;
            if channels.len() == 1 {
                output[target_sample] += channels[0] * gain;
                output[target_sample + 1] += channels[0] * gain;
            } else {
                output[target_sample] += channels[0] * gain;
                output[target_sample + 1] += channels[1] * gain;
            }
        }
    }
    Ok(())
}

fn lowpass(samples: &mut [f32], cutoff: f64, sample_rate: u32) {
    let maximum = f64::from(sample_rate) * 0.49;
    let bounded = cutoff.clamp(20.0_f64.min(maximum), maximum);
    let coefficient = 1.0 - (-TAU * bounded / f64::from(sample_rate)).exp();
    let mut state = [0.0_f64; 2];
    for frame in samples.chunks_exact_mut(2) {
        for channel in 0..2 {
            state[channel] += coefficient * (f64::from(frame[channel]) - state[channel]);
            frame[channel] = state[channel] as f32;
        }
    }
}

fn mix_track(master: &mut [f32], stereo: &[f32], track: &Track) -> Result<(), AudioError> {
    let gain = 10.0_f64.powf(track.gain / 20.0);
    finite(&track.id, "linearGain", gain)?;
    let pan = track.pan.clamp(-1.0, 1.0);
    let left = gain * ((1.0 - pan) * 0.5).sqrt();
    let right = gain * ((1.0 + pan) * 0.5).sqrt();
    finite(&track.id, "panGain", left)?;
    finite(&track.id, "panGain", right)?;
    for (target, source) in master.chunks_exact_mut(2).zip(stereo.chunks_exact(2)) {
        target[0] += (f64::from(source[0]) * left) as f32;
        target[1] += (f64::from(source[1]) * right) as f32;
    }
    Ok(())
}
