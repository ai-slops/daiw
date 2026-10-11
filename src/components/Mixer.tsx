import { useEffect, useState } from "react";
import type { MasterMeter } from "../audio";
import type { Track } from "../domain/project";
import { Panel } from "./primitives";

type TrackPatch = {
	readonly gain?: number;
	readonly pan?: number;
	readonly mute?: boolean;
	readonly solo?: boolean;
};

type ChannelStripProps = {
	readonly track: Track;
	readonly index: number;
	readonly onChange: (patch: TrackPatch) => void;
};

const ChannelStrip = ({ track, index, onChange }: ChannelStripProps) => {
	const [gain, setGain] = useState(track.gain);
	const [pan, setPan] = useState(track.pan);
	useEffect(() => setGain(track.gain), [track.gain]);
	useEffect(() => setPan(track.pan), [track.pan]);
	return (
		<div className="channel-strip">
			<header>
				<span className={`track-dot track-dot--${index % 4}`} />
				<strong>{track.name}</strong>
			</header>
			<span className="channel-strip__routing">POST</span>
			<label className="vertical-fader">
				<span>Gain</span>
				<input
					type="range"
					min="-24"
					max="6"
					step="0.5"
					value={gain}
					onChange={(event) => setGain(event.currentTarget.valueAsNumber)}
					onPointerUp={() => onChange({ gain })}
					onKeyUp={() => onChange({ gain })}
				/>
				<output>{gain.toFixed(1)} dB</output>
			</label>
			<label className="pan-control">
				<span>Pan</span>
				<input
					type="range"
					min="-1"
					max="1"
					step="0.05"
					value={pan}
					onChange={(event) => setPan(event.currentTarget.valueAsNumber)}
					onPointerUp={() => onChange({ pan })}
					onKeyUp={() => onChange({ pan })}
				/>
				<output>
					{pan === 0
						? "C"
						: `${Math.round(Math.abs(pan) * 100)}${pan < 0 ? "L" : "R"}`}
				</output>
			</label>
			<div className="channel-strip__switches">
				<button
					type="button"
					data-active={track.mute}
					onClick={() => onChange({ mute: !track.mute })}
				>
					M
				</button>
				<button
					type="button"
					data-active={track.solo}
					onClick={() => onChange({ solo: !track.solo })}
				>
					S
				</button>
			</div>
		</div>
	);
};

type MixerProps = {
	readonly tracks: readonly Track[];
	readonly onTrack: (trackId: string, patch: TrackPatch) => void;
	readonly meter: MasterMeter;
	readonly meterAvailable: boolean;
};

const meterPercent = (db: number): number =>
	Number.isFinite(db) ? Math.max(0, Math.min(100, ((db + 60) / 60) * 100)) : 0;
const meterLabel = (db: number): string =>
	Number.isFinite(db) ? `${db.toFixed(1)} dB` : "−∞ dB";

export const Mixer = ({
	tracks,
	onTrack,
	meter,
	meterAvailable,
}: MixerProps) => (
	<Panel title="Mixer" eyebrow="Console" className="mixer">
		<div className="mixer__channels">
			{tracks.map((track, index) => (
				<ChannelStrip
					key={track.id}
					track={track}
					index={index}
					onChange={(patch) => onTrack(track.id, patch)}
				/>
			))}
			<div className="master-strip">
				<strong>MASTER</strong>
				{meterAvailable ? (
					<>
						<div
							className="master-meter"
							role="img"
							aria-label={`Master peak ${meterLabel(meter.peakDb)}, RMS ${meterLabel(meter.rmsDb)}`}
						>
							<span style={{ blockSize: `${meterPercent(meter.peakDb)}%` }} />
							<span style={{ blockSize: `${meterPercent(meter.rmsDb)}%` }} />
						</div>
						<output>{meterLabel(meter.peakDb)}</output>
					</>
				) : (
					<p className="master-meter__unavailable">Native meter unavailable</p>
				)}
			</div>
		</div>
	</Panel>
);
