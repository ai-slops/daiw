export const TICKS_PER_BEAT: 960;
export const MAX_BEATS: 1_000_000;

export type TempoPoint = Readonly<{ at: number; bpm: number; curve: "step" | "linear" }>;
export type TempoMap = Readonly<{ points: readonly TempoPoint[] }>;
export type TempoTiming = number | TempoMap;
export type MeterPoint = Readonly<{ at: number; numerator: number; denominator: number }>;
export type MeterMap = Readonly<{ points: readonly MeterPoint[] }>;
export type Meter = readonly [number, number];
export type MeterTiming = Meter | MeterMap;
export type BarPosition = Readonly<{
  bar: number;
  beatInBar: number;
  barStart: number;
  barEnd: number;
}>;

export function validateTempoMap(map: TempoMap): TempoMap;
export function validateMeterMap(map: MeterMap): MeterMap;
export function secondsAtBeat(beat: number, timing: TempoTiming): number;
export function beatAtSeconds(seconds: number, timing: TempoTiming): number;
export function barPositionAtBeat(beat: number, timing: MeterTiming): BarPosition;
export function previousBarBoundary(beat: number, timing: MeterTiming): number;
export function nextBarBoundary(beat: number, timing: MeterTiming): number;
export function isBarBoundary(beat: number, timing: MeterTiming): boolean;
