use crate::dsp_sources::ResidentDspSources;
use crate::render_kernel::{
    KernelError, KernelSource, KernelState, execute_span, validate_program,
};
use crate::source::{SourceMetadata, SourceMetadataProvider};
use crate::{AudioError, CompiledDspProgram, PcmAsset};

const DSP_BLOCK_FRAMES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DspProcessState {
    Running,
    Ended,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DspProcessReport {
    pub state: DspProcessState,
    pub frames: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DspProcessError {
    #[error("DSP source {source_index} has no frame {frame}")]
    SourceFrameUnavailable { source_index: usize, frame: u64 },
    #[error("DSP frame index overflowed")]
    FrameIndexOverflow,
    #[error("DSP track {track} produced a non-finite sample")]
    NonFiniteTrack { track: usize },
    #[error("DSP master produced a non-finite sample")]
    NonFiniteMaster,
}

struct ResidentKernelSources<'a> {
    assets: Vec<&'a PcmAsset>,
}

impl KernelSource for ResidentKernelSources<'_> {
    type Error = DspProcessError;

    fn read_frame(
        &mut self,
        source_index: usize,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<(), Self::Error> {
        let asset =
            self.assets
                .get(source_index)
                .ok_or(DspProcessError::SourceFrameUnavailable {
                    source_index,
                    frame,
                })?;
        let start = usize::try_from(frame)
            .map_err(|_| DspProcessError::FrameIndexOverflow)?
            .checked_mul(usize::from(asset.channels))
            .ok_or(DspProcessError::FrameIndexOverflow)?;
        let end = start
            .checked_add(usize::from(asset.channels))
            .ok_or(DspProcessError::FrameIndexOverflow)?;
        let samples =
            asset
                .samples
                .get(start..end)
                .ok_or(DspProcessError::SourceFrameUnavailable {
                    source_index,
                    frame,
                })?;
        output[..samples.len()].copy_from_slice(samples);
        Ok(())
    }
}

pub struct DspExecutor<'a> {
    program: CompiledDspProgram,
    sources: ResidentKernelSources<'a>,
    filters: Vec<[f64; 2]>,
    master: Box<[[f32; 2]]>,
    track: Box<[[f32; 2]]>,
    source_frame: [f32; 8],
    cursor: usize,
    state: DspProcessState,
    fault: Option<DspProcessError>,
}

impl<'a> DspExecutor<'a> {
    /// Resolves resident assets and allocates all mutable DSP state used by [`Self::process`].
    pub fn prepare(
        program: CompiledDspProgram,
        sources: ResidentDspSources<'a>,
    ) -> Result<Self, AudioError> {
        validate_program(&program)?;
        let mut resolved = Vec::new();
        resolved.try_reserve_exact(program.source_ids.len())?;
        for (id, expected) in program.source_ids.iter().zip(&program.source_metadata) {
            let asset = sources
                .assets
                .get(id)
                .ok_or_else(|| AudioError::MissingAsset {
                    asset_id: id.clone(),
                })?;
            validate_asset(id, asset, *expected, &sources)?;
            resolved.push(asset);
        }
        let mut filters = Vec::new();
        filters.try_reserve_exact(program.track_count())?;
        filters.resize(program.track_count(), [0.0; 2]);
        let state = if program.total_frames() == 0 {
            DspProcessState::Ended
        } else {
            DspProcessState::Running
        };
        Ok(Self {
            program,
            sources: ResidentKernelSources { assets: resolved },
            filters,
            master: vec![[0.0; 2]; DSP_BLOCK_FRAMES].into_boxed_slice(),
            track: vec![[0.0; 2]; DSP_BLOCK_FRAMES].into_boxed_slice(),
            source_frame: [0.0; 8],
            cursor: 0,
            state,
            fault: None,
        })
    }

    /// Renders the next frames without allocating.
    ///
    /// A new fault silences the entire supplied buffer. Chunks completed before that fault remain
    /// committed in [`Self::cursor`]; the same fault is returned until [`Self::reset`] is called.
    pub fn process(
        &mut self,
        output: &mut [[f32; 2]],
    ) -> Result<DspProcessReport, DspProcessError> {
        output.fill([0.0; 2]);
        if let Some(error) = self.fault {
            return Err(error);
        }
        if self.cursor == self.program.total_frames() {
            self.state = DspProcessState::Ended;
            return Ok(DspProcessReport {
                state: self.state,
                frames: 0,
            });
        }
        let mut written = 0;
        while written < output.len() && self.cursor < self.program.total_frames() {
            let frames = DSP_BLOCK_FRAMES
                .min(output.len() - written)
                .min(self.program.total_frames() - self.cursor);
            let result = execute_span(
                &self.program,
                KernelState {
                    master: &mut self.master[..frames],
                    track: &mut self.track[..frames],
                    filters: &mut self.filters,
                    source_frame: &mut self.source_frame,
                },
                &mut self.sources,
                self.cursor,
            );
            if let Err(error) = result {
                let error = map_error(error);
                output.fill([0.0; 2]);
                self.state = DspProcessState::Faulted;
                self.fault = Some(error);
                return Err(error);
            }
            output[written..written + frames].copy_from_slice(&self.master[..frames]);
            self.cursor += frames;
            written += frames;
        }
        self.state = if self.cursor == self.program.total_frames() {
            DspProcessState::Ended
        } else {
            DspProcessState::Running
        };
        Ok(DspProcessReport {
            state: self.state,
            frames: written,
        })
    }

    /// Restarts at frame zero and clears the sticky fault, filters, and scratch buffers.
    pub fn reset(&mut self) {
        self.filters.fill([0.0; 2]);
        self.master.fill([0.0; 2]);
        self.track.fill([0.0; 2]);
        self.source_frame.fill(0.0);
        self.cursor = 0;
        self.state = if self.program.total_frames() == 0 {
            DspProcessState::Ended
        } else {
            DspProcessState::Running
        };
        self.fault = None;
    }

    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub const fn state(&self) -> DspProcessState {
        self.state
    }

    #[must_use]
    pub const fn program(&self) -> &CompiledDspProgram {
        &self.program
    }
}

fn validate_asset(
    id: &str,
    asset: &PcmAsset,
    expected: SourceMetadata,
    provider: &impl SourceMetadataProvider,
) -> Result<(), AudioError> {
    if !(1..=8).contains(&asset.channels) {
        return Err(AudioError::UnsupportedPcmChannels {
            channels: asset.channels,
        });
    }
    if asset.sample_rate == 0 {
        return Err(AudioError::InvalidAssetChannels {
            asset_id: id.to_owned(),
            channels: asset.channels,
        });
    }
    if !asset
        .samples
        .len()
        .is_multiple_of(usize::from(asset.channels))
    {
        return Err(AudioError::InvalidAssetLayout {
            asset_id: id.to_owned(),
            channels: asset.channels,
        });
    }
    if provider.metadata(id)? != expected {
        return Err(AudioError::PcmMetadataMismatch);
    }
    if asset.samples.iter().any(|sample| !sample.is_finite()) {
        return Err(AudioError::InvalidValue {
            entity_id: id.to_owned(),
            field: "samples",
        });
    }
    Ok(())
}

const fn map_error(error: KernelError<DspProcessError>) -> DspProcessError {
    match error {
        KernelError::Source(error) => error,
        KernelError::FrameIndexOverflow => DspProcessError::FrameIndexOverflow,
        KernelError::NonFiniteTrack { track } => DspProcessError::NonFiniteTrack { track },
        KernelError::NonFiniteMaster => DspProcessError::NonFiniteMaster,
    }
}
