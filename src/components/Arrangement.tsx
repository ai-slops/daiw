import { Copy, GripVertical, LockKeyhole, Trash2 } from "lucide-react";
import type { CSSProperties } from "react";
import { useEffect, useMemo, useState } from "react";
import type { Project } from "../domain/project";
import { getWaveform } from "../lib/api-client";
import { timelineBeats, timelinePercent } from "./arrangement-layout";
import { EmptyState, IconButton } from "./primitives";

type ArrangementProps = {
	readonly project: Project;
	readonly positionBeat: number;
	readonly selectedInstanceId: string | undefined;
	readonly onSelect: (instanceId: string) => void;
	readonly onDuplicate: (instanceId: string) => void;
	readonly onDelete: (instanceId: string) => void;
};

export const Arrangement = ({
	project,
	positionBeat,
	selectedInstanceId,
	onSelect,
	onDuplicate,
	onDelete,
}: ArrangementProps) => {
	const beatsVisible = timelineBeats(project);
	const ruler = useMemo(
		() => Array.from({ length: beatsVisible / 4 + 1 }, (_, index) => index * 4),
		[beatsVisible],
	);
	const [waveforms, setWaveforms] = useState<
		Readonly<Record<string, readonly number[]>>
	>({});
	const [waveformErrors, setWaveformErrors] = useState<
		Readonly<Record<string, string>>
	>({});
	useEffect(() => {
		let active = true;
		const assetIds = [
			...new Set(project.regions.map((region) => region.assetId)),
		];
		setWaveformErrors({});
		void Promise.allSettled(
			assetIds.map(
				async (assetId) =>
					[assetId, await getWaveform(project.id, assetId)] as const,
			),
		).then((results) => {
			if (!active) return;
			const entries: Array<readonly [string, readonly number[]]> = [];
			const errors: Record<string, string> = {};
			results.forEach((result, index) => {
				const assetId = assetIds[index];
				if (assetId === undefined) return;
				if (result.status === "fulfilled") entries.push(result.value);
				else
					errors[assetId] =
						result.reason instanceof Error
							? result.reason.message
							: "Waveform unavailable";
			});
			setWaveforms(Object.fromEntries(entries));
			setWaveformErrors(errors);
		});
		return () => {
			active = false;
		};
	}, [project.id, project.regions]);

	return (
		<section className="arrangement" aria-label="Arrangement timeline">
			<div className="timeline-ruler" aria-hidden="true">
				<span className="timeline-ruler__corner">TRACK / TIME</span>
				<div className="timeline-ruler__beats">
					{ruler.map((beat) => (
						<span
							key={beat}
							style={{ insetInlineStart: timelinePercent(beat, beatsVisible) }}
						>
							{beat / 4 + 1}
						</span>
					))}
				</div>
			</div>
			<div className="track-field">
				<span
					className="arrangement-playhead"
					style={{
						insetInlineStart: `calc(var(--track-head-width) + (100% - var(--track-head-width)) * ${Math.max(0, Math.min(positionBeat, beatsVisible)) / beatsVisible})`,
					}}
					aria-hidden="true"
				/>
				{project.tracks.length === 0 ? (
					<EmptyState>Add a track to start arranging.</EmptyState>
				) : null}
				{project.tracks.map((track, trackIndex) => {
					const clips = project.instances.filter(
						(instance) => instance.trackId === track.id,
					);
					const regions = project.regions.filter(
						(region) => region.trackId === track.id,
					);
					return (
						<div className="track-row" key={track.id}>
							<div className="track-head">
								<span
									className={`track-dot track-dot--${trackIndex % 4}`}
									aria-hidden="true"
								/>
								<div>
									<strong>{track.name}</strong>
									<small>
										{track.role} · {track.instrument}
									</small>
								</div>
								<span className="track-state">
									{track.mute ? "M" : track.solo ? "S" : ""}
								</span>
							</div>
							<div className="track-lane">
								{clips.map((clip) => {
									const style: CSSProperties = {
										insetInlineStart: timelinePercent(clip.start, beatsVisible),
										inlineSize: timelinePercent(clip.length, beatsVisible),
									};
									return (
										<button
											key={clip.id}
											type="button"
											className={`clip clip--${trackIndex % 4}`}
											data-selected={clip.id === selectedInstanceId}
											style={style}
											onClick={() => onSelect(clip.id)}
											aria-label={`${clip.name}, starts at beat ${clip.start}, ${clip.length} beats${clip.modifiers.length > 0 ? ", modified" : ""}`}
										>
											<GripVertical aria-hidden="true" size="var(--icon-sm)" />
											<span>
												<strong>{clip.name}</strong>
												<small>
													{clip.modifiers.length > 0
														? `${clip.modifiers.length} MOD`
														: "LINKED"}
												</small>
											</span>
											{clip.overrides.length > 0 ? (
												<LockKeyhole
													aria-label="Has instance overrides"
													size="var(--icon-sm)"
												/>
											) : (
												<span className="clip__notes" aria-hidden="true" />
											)}
										</button>
									);
								})}
								{regions.map((region) => {
									const asset = project.assets.find(
										(item) => item.id === region.assetId,
									);
									const peaks = waveforms[region.assetId] ?? [];
									const waveformError = waveformErrors[region.assetId];
									return (
										<div
											key={region.id}
											className={`audio-region audio-region--${trackIndex % 4}`}
											role="img"
											style={{
												insetInlineStart: timelinePercent(
													region.start,
													beatsVisible,
												),
												inlineSize: timelinePercent(
													region.duration,
													beatsVisible,
												),
											}}
											aria-label={`${asset?.name ?? "Audio region"}, starts at beat ${region.start}, ${region.duration} beats`}
										>
											<strong>{asset?.name ?? "Audio"}</strong>
											{waveformError === undefined ? null : (
												<small
													className="audio-region__error"
													title={waveformError}
												>
													Waveform unavailable
												</small>
											)}
											<svg
												viewBox="0 0 128 24"
												preserveAspectRatio="none"
												aria-hidden="true"
											>
												<path
													d={peaks
														.map(
															(peak, index) =>
																`M${index} ${12 - peak * 11}V${12 + peak * 11}`,
														)
														.join("")}
												/>
											</svg>
										</div>
									);
								})}
							</div>
						</div>
					);
				})}
			</div>
			{selectedInstanceId === undefined ? null : (
				<fieldset
					className="selection-actions"
					aria-label="Selected clip actions"
				>
					<IconButton
						label="Duplicate selected clip"
						icon={Copy}
						onClick={() => onDuplicate(selectedInstanceId)}
					/>
					<IconButton
						label="Delete selected clip"
						icon={Trash2}
						variant="danger"
						onClick={() => onDelete(selectedInstanceId)}
					/>
				</fieldset>
			)}
		</section>
	);
};
