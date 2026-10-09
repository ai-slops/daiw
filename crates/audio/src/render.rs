use crate::render_clock::RenderClock;
use crate::render_kernel::{Collector, RenderContext, render_blocks};
use crate::source::{ResidentSourceBank, SourceBank};
use crate::validate::{validate_request, validate_snapshot};
use crate::{AudioError, RenderRequest, RenderedAudio, Snapshot};

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub(crate) const OUTPUT_BLOCK_FRAMES: usize = 4096;

pub fn render(request: &RenderRequest) -> Result<RenderedAudio, AudioError> {
    render_with_sample_rate(request, DEFAULT_SAMPLE_RATE)
}

pub fn render_with_sample_rate(
    request: &RenderRequest,
    sample_rate: u32,
) -> Result<RenderedAudio, AudioError> {
    render_with_sample_rate_and_block_size::<OUTPUT_BLOCK_FRAMES>(request, sample_rate)
}

fn render_with_sample_rate_and_block_size<const BLOCK_FRAMES: usize>(
    request: &RenderRequest,
    sample_rate: u32,
) -> Result<RenderedAudio, AudioError> {
    validate_request(request, sample_rate)?;
    let mut sources = ResidentSourceBank::new(&request.assets);
    collect_from_sources::<BLOCK_FRAMES>(&request.snapshot, &mut sources, sample_rate)
}

pub(crate) fn render_file_sources(
    snapshot: &Snapshot,
    sources: &mut impl SourceBank,
) -> Result<RenderedAudio, AudioError> {
    validate_snapshot(snapshot, DEFAULT_SAMPLE_RATE)?;
    collect_from_sources::<OUTPUT_BLOCK_FRAMES>(snapshot, sources, DEFAULT_SAMPLE_RATE)
}

fn collect_from_sources<const BLOCK_FRAMES: usize>(
    snapshot: &Snapshot,
    sources: &mut impl SourceBank,
    sample_rate: u32,
) -> Result<RenderedAudio, AudioError> {
    let clock = RenderClock::new(snapshot, sample_rate)?;
    let mut collector = Collector::new(clock.total_frames)?;
    render_blocks::<BLOCK_FRAMES>(
        RenderContext {
            snapshot,
            clock: &clock,
        },
        sources,
        &mut collector,
    )?;
    Ok(RenderedAudio {
        sample_rate,
        channels: 2,
        samples: collector.into_samples(),
    })
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
