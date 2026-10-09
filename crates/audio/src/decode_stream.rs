use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::AudioError;
use crate::pcm_format::{
    PCM_HEADER_LEN, PCM_PEAK_COUNT, PcmDescriptor, PcmHeader, lowercase_hex, validate_metadata,
};

const MAX_DECODED_PACKET_BYTES: usize = 8 * 1024 * 1024;

struct IncompleteOutput {
    path: PathBuf,
    complete: bool,
}

impl Drop for IncompleteOutput {
    fn drop(&mut self) {
        if !self.complete {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub fn decode_to_pcm(input: &Path, output: &Path) -> Result<PcmDescriptor, AudioError> {
    if !input.is_absolute() || !output.is_absolute() {
        return Err(AudioError::InvalidDecodeFilePath);
    }
    let source = File::open(input)?;
    let mut destination = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(output)?;
    let mut cleanup = IncompleteOutput {
        path: output.to_owned(),
        complete: false,
    };
    destination.write_all(&[0_u8; PCM_HEADER_LEN])?;

    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = input.extension().and_then(|value| value.to_str()) {
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
    let mut metadata: Option<(u32, u16)> = None;
    let mut frames = 0_u64;
    let mut payload_hasher = Sha256::new();
    {
        let mut writer = BufWriter::new(&mut destination);
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
            // Symphonia owns the codec's decoded packet allocation. This boundary limits the
            // additional interleaved conversion buffer allocated below.
            let decoded = decoder
                .decode(&packet)
                .map_err(|error| AudioError::Decode(error.to_string()))?;
            let spec = *decoded.spec();
            let channels = u16::try_from(spec.channels.count())?;
            let packet_bytes = decoded
                .frames()
                .checked_mul(spec.channels.count())
                .and_then(|samples| samples.checked_mul(4))
                .ok_or(AudioError::DecodedPacketTooLarge)?;
            if packet_bytes > MAX_DECODED_PACKET_BYTES {
                return Err(AudioError::DecodedPacketTooLarge);
            }
            match metadata {
                Some((rate, count)) if rate != spec.rate || count != channels => {
                    return Err(AudioError::ChangingPcmFormat);
                }
                Some(_) => {}
                None => metadata = Some((spec.rate, channels)),
            }
            let duration = u64::try_from(decoded.frames())?;
            let next_frames = frames
                .checked_add(duration)
                .ok_or(AudioError::PcmPayloadTooLarge)?;
            let payload_bytes = next_frames
                .checked_mul(u64::from(channels))
                .and_then(|samples| samples.checked_mul(4))
                .ok_or(AudioError::PcmPayloadTooLarge)?;
            validate_metadata(spec.rate, channels, next_frames, payload_bytes)?;
            let mut converted = SampleBuffer::<f32>::new(duration, spec);
            converted.copy_interleaved_ref(decoded);
            frames = next_frames;
            for sample in converted.samples() {
                if !sample.is_finite() {
                    return Err(AudioError::NonFiniteDecodedPcm);
                }
                let bytes = sample.to_le_bytes();
                writer.write_all(&bytes)?;
                payload_hasher.update(bytes);
            }
        }
        writer.flush()?;
    }
    let (sample_rate, channels) = metadata.ok_or(AudioError::NoAudioTrack)?;
    let payload_bytes = frames
        .checked_mul(u64::from(channels))
        .and_then(|samples| samples.checked_mul(4))
        .ok_or(AudioError::PcmPayloadTooLarge)?;
    let header = PcmHeader {
        sample_rate,
        channels,
        frames,
        payload_bytes,
        payload_sha256: payload_hasher.finalize().into(),
    };
    destination.seek(SeekFrom::Start(0))?;
    destination.write_all(&header.encode())?;
    destination.flush()?;
    destination.sync_all()?;
    drop(destination);

    let (sha256, peaks) = scan_final_file(output, &header)?;
    cleanup.complete = true;
    Ok(PcmDescriptor {
        format: "daiw-f32le-v1",
        frames,
        sample_rate,
        channels,
        byte_length: header.byte_length()?,
        sha256,
        peaks,
    })
}

fn scan_final_file(path: &Path, header: &PcmHeader) -> Result<(String, Vec<f32>), AudioError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut header_bytes = [0_u8; PCM_HEADER_LEN];
    reader.read_exact(&mut header_bytes)?;
    let mut whole_hasher = Sha256::new();
    whole_hasher.update(header_bytes);
    let mut peaks = vec![0.0_f32; PCM_PEAK_COUNT];
    let frame_bytes = usize::from(header.channels) * 4;
    let mut frame = [0_u8; 32];
    for frame_index in 0..header.frames {
        let bytes = &mut frame[..frame_bytes];
        reader.read_exact(bytes)?;
        whole_hasher.update(&*bytes);
        let bucket = usize::try_from(
            frame_index.saturating_mul(PCM_PEAK_COUNT as u64) / header.frames.max(1),
        )?
        .min(PCM_PEAK_COUNT - 1);
        for encoded in bytes.chunks_exact(4) {
            let sample = f32::from_le_bytes(encoded.try_into()?);
            peaks[bucket] = peaks[bucket].max(sample.abs());
        }
    }
    let mut trailing = [0_u8; 1];
    if reader.read(&mut trailing)? != 0 {
        return Err(AudioError::InvalidPcmLength);
    }
    Ok((lowercase_hex(&whole_hasher.finalize()), peaks))
}
