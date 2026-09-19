import {
	Download,
	FastForward,
	Pause,
	Play,
	Repeat2,
	Rewind,
	Square,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { AudioEngineState } from "../audio";
import { IconButton } from "./primitives";

type TransportProps = {
	readonly tempo: number;
	readonly onTempo: (tempo: number) => void;
	readonly audio: AudioEngineState;
	readonly busy: boolean;
	readonly onToggle: () => void;
	readonly onStop: () => void;
	readonly onSeek: (beat: number) => void;
	readonly onLoop: () => void;
	readonly onRender: () => void;
};

export const Transport = ({
	tempo,
	onTempo,
	audio,
	busy,
	onToggle,
	onStop,
	onSeek,
	onLoop,
	onRender,
}: TransportProps) => {
	const playing = audio.status === "playing";
	const position = audio.positionBeat;
	const [tempoDraft, setTempoDraft] = useState(String(tempo));
	const editingTempo = useRef(false);
	useEffect(() => {
		if (!editingTempo.current) setTempoDraft(String(tempo));
	}, [tempo]);
	const commitTempo = () => {
		editingTempo.current = false;
		const nextTempo = Number(tempoDraft);
		if (!Number.isFinite(nextTempo) || nextTempo < 30 || nextTempo > 300) {
			setTempoDraft(String(tempo));
			return;
		}
		if (nextTempo !== tempo) onTempo(nextTempo);
	};
	return (
		<section className="transport" aria-label="Transport controls">
			<div className="transport__group">
				<IconButton
					label="Jump to start"
					icon={Rewind}
					onClick={() => onSeek(0)}
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
					label="Jump forward four bars"
					icon={FastForward}
					onClick={() => onSeek(position + 16)}
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
			<output className="transport__position" aria-label="Current position">
				{String(Math.floor(position / 4) + 1).padStart(3, "0")} <span>/</span>{" "}
				{String(Math.floor(position % 4) + 1).padStart(2, "0")} <span>/</span>{" "}
				000
			</output>
			<label className="tempo-control">
				<span>TEMPO</span>
				<input
					type="number"
					min="30"
					max="300"
					value={tempoDraft}
					onFocus={() => {
						editingTempo.current = true;
					}}
					onChange={(event) => setTempoDraft(event.currentTarget.value)}
					onBlur={commitTempo}
					onKeyDown={(event) => {
						if (event.key === "Enter") event.currentTarget.blur();
					}}
				/>
				<small>BPM</small>
			</label>
			<div className="meter-control">
				<span>4 / 4</span>
				<small>METER</small>
			</div>
			<div className="transport__engine">
				<span className="status-dot" />
				48 kHz
			</div>
		</section>
	);
};
