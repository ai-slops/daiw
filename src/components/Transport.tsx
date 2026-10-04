import {
	Download,
	FastForward,
	Pause,
	Play,
	Repeat2,
	Rewind,
	SlidersHorizontal,
	Square,
} from "lucide-react";
import { secondsAtBeat } from "../../shared/timing.mjs";
import type { AudioEngineState } from "../audio";
import type { Project } from "../domain/project";
import { IconButton } from "./primitives";
import {
	formatMusicalPosition,
	meterAtBeat,
	nextBar,
	previousBar,
} from "./transport-timing";

type TransportProps = {
	readonly timing: Pick<Project, "tempo" | "meter" | "tempoMap" | "meterMap">;
	readonly audio: AudioEngineState;
	readonly busy: boolean;
	readonly onToggle: () => void;
	readonly onStop: () => void;
	readonly onSeek: (beat: number) => void;
	readonly onLoop: () => void;
	readonly onRender: () => void;
	readonly onTiming: () => void;
};

export const Transport = ({
	timing,
	audio,
	busy,
	onToggle,
	onStop,
	onSeek,
	onLoop,
	onRender,
	onTiming,
}: TransportProps) => {
	const playing = audio.status === "playing";
	const position = audio.positionBeat;
	const meterTiming = timing.meterMap ?? timing.meter;
	const tempoTiming = timing.tempoMap ?? timing.tempo;
	const [numerator, denominator] = meterAtBeat(position, meterTiming);
	const musicalPosition = formatMusicalPosition(position, meterTiming);
	return (
		<section className="transport" aria-label="Transport controls">
			<div className="transport__group">
				<IconButton
					label="Previous bar"
					icon={Rewind}
					onClick={() => onSeek(previousBar(position, meterTiming))}
				/>
				<IconButton
					label={playing ? "Pause playback" : "Start playback"}
					icon={playing ? Pause : Play}
					variant="primary"
					onClick={onToggle}
					disabled={busy}
				/>
				<IconButton label="Stop playback" icon={Square} onClick={onStop} />
				<IconButton
					label="Next bar"
					icon={FastForward}
					onClick={() => onSeek(nextBar(position, meterTiming))}
				/>
				<IconButton
					label="Loop current four bars"
					icon={Repeat2}
					aria-pressed={audio.loop !== null}
					onClick={onLoop}
				/>
				<IconButton
					label="Download master WAV"
					icon={Download}
					onClick={onRender}
				/>
			</div>
			<output
				className="transport__position"
				aria-label="Current musical position"
				title={`${secondsAtBeat(position, tempoTiming).toFixed(2)} seconds`}
			>
				{musicalPosition}
			</output>
			<button type="button" className="tempo-control" onClick={onTiming}>
				<SlidersHorizontal aria-hidden="true" size="var(--icon-sm)" />
				<span>
					{timing.tempo} BPM{timing.tempoMap === undefined ? "" : " · MAP"}
				</span>
			</button>
			<div className="meter-control">
				<span>
					{numerator} / {denominator}
				</span>
				<small>METER</small>
			</div>
			<div className="transport__engine">
				<span className="status-dot" />
				48 kHz
			</div>
		</section>
	);
};
