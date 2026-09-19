import { loudnessMatchGain, updateMeter } from "./analysis";
import { fetchServerRender, snapshotRevisionMatches } from "./server-render";
import { beatAtTime, normalizeLoopBeat, secondsAtBeat, validateLoop } from "./transport";
import { snapshotSchema } from "./types";
import type {
  AudioEngineState,
  AuditionSide,
  LoopRange,
  MasterMeter,
  Snapshot,
  StateListener,
  TransportStatus,
} from "./types";

export class AudioEngineError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "AudioEngineError";
  }
}

export class DaiwAudioEngine {
  readonly #context: AudioContext;
  readonly #listeners = new Set<StateListener>();
  readonly #auditions = new Map<AuditionSide, AudioBuffer>();
  readonly #analyser: AnalyserNode;
  readonly #meterSamples: Float32Array<ArrayBuffer>;
  readonly #meter = { peak: 0, rms: 0, peakDb: -Infinity, rmsDb: -Infinity };
  #snapshot: Snapshot | null = null;
  #status: TransportStatus = "stopped";
  #positionBeat = 0;
  #startedAt = 0;
  #loop: LoopRange | null = null;
  #audition: AuditionSide | null = null;
  #source: AudioBufferSourceNode | null = null;
  #gain: GainNode | null = null;
  #loudnessMatching = false;

  public constructor(context: AudioContext = new AudioContext()) {
    this.#context = context;
    this.#analyser = context.createAnalyser();
    this.#analyser.fftSize = 2_048;
    this.#analyser.smoothingTimeConstant = 0;
    this.#meterSamples = new Float32Array(this.#analyser.fftSize);
    this.#analyser.connect(context.destination);
  }

  public setSnapshot(input: unknown): Snapshot {
    this.stop();
    this.#auditions.clear();
    this.#audition = null;
    this.#loop = null;
    const snapshot = Object.freeze(snapshotSchema.parse(input));
    this.#snapshot = snapshot;
    this.#emit();
    return snapshot;
  }

  public async prepare(projectId: string): Promise<void> {
    const expectedRevision = this.#requireSnapshot().revision;
    const buffer = await fetchServerRender(this.#context, { projectId, expectedRevision });
    this.#assertCurrentRevision(expectedRevision);
    this.#auditions.set("a", buffer);
    this.selectAB("a");
  }

  public async prepareAB(projectId: string, proposalId: string): Promise<void> {
    const expectedRevision = this.#requireSnapshot().revision;
    const [a, b] = await Promise.all([
      fetchServerRender(this.#context, { projectId, expectedRevision }),
      fetchServerRender(this.#context, { projectId, expectedRevision, proposalId }),
    ]);
    this.#assertCurrentRevision(expectedRevision);
    this.#auditions.set("a", a);
    this.#auditions.set("b", b);
    this.selectAB("a");
  }

  public async play(): Promise<void> {
    this.#requireSnapshot();
    this.#requireBuffer();
    if (this.#status === "playing") return;
    await this.#context.resume();
    this.#status = "playing";
    this.#startedAt = this.#context.currentTime;
    this.#schedule();
    this.#emit();
  }

  public pause(): void {
    if (this.#status !== "playing") return;
    this.#positionBeat = this.getPositionBeats();
    this.#status = "paused";
    this.#clearSource();
    this.#emit();
  }

  public stop(): void {
    this.#clearSource();
    this.#status = "stopped";
    this.#positionBeat = 0;
    this.#emit();
  }

  public seek(beat: number): void {
    const snapshot = this.#requireSnapshot();
    this.#positionBeat = Math.max(0, Math.min(beat, snapshot.length));
    if (this.#status === "playing") {
      this.#clearSource();
      this.#startedAt = this.#context.currentTime;
      this.#schedule();
    }
    this.#emit();
  }

  public setLoop(loop: LoopRange | null): void {
    const snapshot = this.#requireSnapshot();
    this.#loop = loop === null ? null : validateLoop(loop, snapshot.length);
    if (this.#status === "playing") this.seek(this.getPositionBeats());
    this.#emit();
  }

  public setLoudnessMatching(enabled: boolean): void {
    this.#loudnessMatching = enabled;
    if (this.#status === "playing") this.seek(this.getPositionBeats());
    this.#emit();
  }

  public selectAB(side: AuditionSide): void {
    if (!this.#auditions.has(side)) throw new AudioEngineError(`audition ${side} is not prepared`);
    const playing = this.#status === "playing";
    if (playing) this.#positionBeat = this.getPositionBeats();
    this.#clearSource();
    this.#audition = side;
    if (playing) {
      this.#startedAt = this.#context.currentTime;
      this.#schedule();
    }
    this.#emit();
  }

  public getPositionBeats(): number {
    const snapshot = this.#snapshot;
    if (snapshot === null || this.#status !== "playing") return this.#positionBeat;
    const elapsed = this.#context.currentTime - this.#startedAt;
    const position = this.#positionBeat + beatAtTime(elapsed, snapshot.tempo);
    return Math.min(snapshot.length, normalizeLoopBeat(position, this.#loop));
  }

  public getState(): AudioEngineState {
    return {
      status: this.#status,
      positionBeat: this.getPositionBeats(),
      loop: this.#loop,
      revision: this.#snapshot?.revision ?? null,
      audition: this.#audition,
      loudnessMatching: this.#loudnessMatching,
    };
  }

  public getMeter(): MasterMeter {
    if (this.#status === "playing") {
      this.#analyser.getFloatTimeDomainData(this.#meterSamples);
    } else {
      this.#meterSamples.fill(0);
    }
    return updateMeter(this.#meterSamples, this.#meter);
  }

  public subscribe(listener: StateListener): () => void {
    this.#listeners.add(listener);
    listener(this.getState());
    return () => this.#listeners.delete(listener);
  }

  #schedule(): void {
    const snapshot = this.#requireSnapshot();
    const buffer = this.#requireBuffer();
    const source = this.#context.createBufferSource();
    const gain = this.#context.createGain();
    source.buffer = buffer;
    if (this.#loop !== null) {
      source.loop = true;
      source.loopStart = secondsAtBeat(this.#loop.start, snapshot.tempo);
      source.loopEnd = secondsAtBeat(this.#loop.end, snapshot.tempo);
    }
    gain.gain.value = this.#selectedGain(buffer);
    source.connect(gain).connect(this.#analyser);
    source.onended = () => this.#handleEnded(source, snapshot.length);
    source.start(0, secondsAtBeat(this.#positionBeat, snapshot.tempo));
    this.#source = source;
    this.#gain = gain;
  }

  #selectedGain(selected: AudioBuffer): number {
    const reference = this.#auditions.get("a");
    return this.#loudnessMatching && this.#audition === "b" && reference !== undefined
      ? loudnessMatchGain(reference, selected)
      : 1;
  }

  #handleEnded(source: AudioBufferSourceNode, endBeat: number): void {
    if (source !== this.#source || this.#loop !== null) return;
    this.#source = null;
    this.#gain = null;
    this.#positionBeat = endBeat;
    this.#status = "stopped";
    this.#emit();
  }

  #clearSource(): void {
    if (this.#source === null) return;
    this.#source.onended = null;
    this.#source.stop();
    this.#source.disconnect();
    this.#gain?.disconnect();
    this.#source = null;
    this.#gain = null;
  }

  #requireSnapshot(): Snapshot {
    if (this.#snapshot === null) throw new AudioEngineError("setSnapshot must be called first");
    return this.#snapshot;
  }

  #assertCurrentRevision(captured: string): void {
    const current = this.#snapshot?.revision ?? null;
    if (!snapshotRevisionMatches(captured, current)) {
      throw new AudioEngineError(
        `snapshot changed while preparing audio: captured ${captured}, current ${current ?? "missing"}`,
      );
    }
  }

  #requireBuffer(): AudioBuffer {
    if (this.#audition === null) throw new AudioEngineError("prepare must complete before playback");
    const buffer = this.#auditions.get(this.#audition);
    if (buffer === undefined) throw new AudioEngineError("selected server render is unavailable");
    return buffer;
  }

  #emit(): void {
    const state = this.getState();
    for (const listener of this.#listeners) listener(state);
  }
}
