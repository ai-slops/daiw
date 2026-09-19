use std::{fs::File, path::Path};

use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error as SymphoniaError,
    formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

use crate::{AudioError, DecodedAsset};

pub const MAX_DECODED_SAMPLES: usize = 4_000_000;

pub fn decode_file(path: &Path) -> Result<DecodedAsset, AudioError> {
    if !path.is_absolute() {
        return Err(AudioError::InvalidDecodePath);
    }
    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|error| AudioError::Decode(error.to_string()))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or(AudioError::NoAudioTrack)?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|error| AudioError::Decode(error.to_string()))?;
    let mut samples = Vec::new();
    let mut sample_rate = None;
    let mut channels = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(AudioError::Decode(error.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder
            .decode(&packet)
            .map_err(|error| AudioError::Decode(error.to_string()))?;
        let spec = *decoded.spec();
        let packet_samples = decoded.capacity().saturating_mul(spec.channels.count());
        let attempted = samples.len().saturating_add(packet_samples);
        if attempted > MAX_DECODED_SAMPLES {
            return Err(AudioError::ResourceLimit {
                limit: MAX_DECODED_SAMPLES,
                attempted,
            });
        }
        sample_rate = Some(spec.rate);
        channels = u16::try_from(spec.channels.count()).ok();
        let duration = u64::try_from(decoded.capacity())
            .map_err(|error| AudioError::Decode(error.to_string()))?;
        let mut converted = SampleBuffer::<f32>::new(duration, spec);
        converted.copy_interleaved_ref(decoded);
        let attempted = samples.len().saturating_add(converted.samples().len());
        if attempted > MAX_DECODED_SAMPLES {
            return Err(AudioError::ResourceLimit {
                limit: MAX_DECODED_SAMPLES,
                attempted,
            });
        }
        samples.extend_from_slice(converted.samples());
    }
    let sample_rate = sample_rate.ok_or(AudioError::NoAudioTrack)?;
    let channels = channels.ok_or(AudioError::NoAudioTrack)?;
    let frames = samples.len() / usize::from(channels);
    Ok(DecodedAsset {
        frames,
        sample_rate,
        channels,
        samples,
    })
}
