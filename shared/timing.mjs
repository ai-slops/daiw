export const TICKS_PER_BEAT = 960;
export const MAX_BEATS = 1_000_000;

const TICK_EPSILON = 1e-7;
const BEAT_EPSILON = TICK_EPSILON / TICKS_PER_BEAT;

function requireNonnegativeFinite(value, name) {
  if (!Number.isFinite(value) || value < 0) throw new RangeError(`${name} must be a nonnegative finite number`);
}

function requireBeat(beat, name) {
  requireNonnegativeFinite(beat, name);
  if (beat > MAX_BEATS) throw new RangeError(`${name} must not exceed ${MAX_BEATS}`);
}

function requireBpm(bpm) {
  if (!Number.isFinite(bpm) || bpm < 20 || bpm > 400) throw new RangeError('tempo must be between 20 and 400 bpm');
}

function roundedTick(beat) {
  requireBeat(beat, 'beat');
  const tick = Math.round(beat * TICKS_PER_BEAT);
  if (Math.abs(beat * TICKS_PER_BEAT - tick) > TICK_EPSILON) throw new RangeError('beat must be tick-aligned');
  return tick;
}

export function validateTempoMap(map) {
  if (map === null || typeof map !== 'object' || !Array.isArray(map.points) || map.points.length < 1 || map.points.length > 1024) {
    throw new RangeError('tempo map must contain 1 to 1024 points');
  }
  let previousTick = -1;
  for (const point of map.points) {
    if (point === null || typeof point !== 'object') throw new RangeError('tempo point must be an object');
    const tick = roundedTick(point.at);
    requireBpm(point.bpm);
    if (tick <= previousTick) throw new RangeError('tempo points must be tick-aligned and strictly increasing');
    if (point.curve !== 'step' && point.curve !== 'linear') throw new RangeError('tempo curve must be step or linear');
    previousTick = tick;
  }
  if (map.points[0].at !== 0) throw new RangeError('tempo map must start at beat zero');
  if (map.points.at(-1).curve !== 'step') throw new RangeError('last tempo point must use a step curve');
  return map;
}

export function validateMeterMap(map) {
  if (map === null || typeof map !== 'object' || !Array.isArray(map.points) || map.points.length < 1 || map.points.length > 1024) {
    throw new RangeError('meter map must contain 1 to 1024 points');
  }
  let previous = null;
  let previousTick = -1;
  for (const point of map.points) {
    if (point === null || typeof point !== 'object') throw new RangeError('meter point must be an object');
    const tick = roundedTick(point.at);
    if (tick <= previousTick) {
      throw new RangeError('meter points must be tick-aligned and strictly increasing');
    }
    if (!Number.isInteger(point.numerator) || point.numerator < 1 || point.numerator > 255 ||
        !Number.isInteger(point.denominator) || point.denominator < 1 || point.denominator > 255) {
      throw new RangeError('meter values must be positive 8-bit integers');
    }
    if (previous !== null) {
      const barUnits = previous.numerator * 4 * TICKS_PER_BEAT;
      if (((tick - previousTick) * previous.denominator) % barUnits !== 0) {
        throw new RangeError('meter change must fall on a previous-meter bar boundary');
      }
    }
    previous = point;
    previousTick = tick;
  }
  if (map.points[0].at !== 0) throw new RangeError('meter map must start at beat zero');
  return map;
}

function normalizeTempo(timing) {
  if (typeof timing === 'number') {
    requireBpm(timing);
    return { points: [{ at: 0, bpm: timing, curve: 'step' }] };
  }
  return validateTempoMap(timing);
}

function normalizeMeter(timing) {
  if (Array.isArray(timing)) {
    const map = { points: [{ at: 0, numerator: timing[0], denominator: timing[1] }] };
    return validateMeterMap(map);
  }
  return validateMeterMap(timing);
}

function segmentSeconds(beats, segmentLength, fromBpm, toBpm, curve) {
  if (curve === 'step' || fromBpm === toBpm) return beats * 60 / fromBpm;
  const slope = (toBpm - fromBpm) / segmentLength;
  return 60 / slope * Math.log1p(slope * beats / fromBpm);
}

function segmentBeats(seconds, length, fromBpm, toBpm, curve) {
  if (curve === 'step' || fromBpm === toBpm) return seconds * fromBpm / 60;
  const slope = (toBpm - fromBpm) / length;
  return fromBpm * Math.expm1(seconds * slope / 60) / slope;
}

export function secondsAtBeat(beat, timing) {
  requireBeat(beat, 'beat');
  const { points } = normalizeTempo(timing);
  let seconds = 0;
  for (let index = 0; index < points.length; index += 1) {
    const point = points[index];
    const next = points[index + 1];
    const end = next === undefined ? beat : Math.min(beat, next.at);
    const length = end - point.at;
    if (length > 0) seconds += next === undefined ? length * 60 / point.bpm : segmentSeconds(length, next.at - point.at, point.bpm, next.bpm, point.curve);
    if (next === undefined || beat <= next.at) return seconds;
  }
  return seconds;
}

export function beatAtSeconds(seconds, timing) {
  requireNonnegativeFinite(seconds, 'seconds');
  const { points } = normalizeTempo(timing);
  let remaining = seconds;
  for (let index = 0; index < points.length; index += 1) {
    const point = points[index];
    const next = points[index + 1];
    if (next === undefined) {
      const beat = point.at + remaining * point.bpm / 60;
      requireBeat(beat, 'beat');
      return beat;
    }
    const length = next.at - point.at;
    const duration = segmentSeconds(length, length, point.bpm, next.bpm, point.curve);
    if (remaining <= duration) return point.at + segmentBeats(remaining, length, point.bpm, next.bpm, point.curve);
    remaining -= duration;
  }
  return 0;
}

function barLength(point) {
  return point.numerator * 4 / point.denominator;
}

function meterSegmentAt(beat, timing) {
  const { points } = normalizeMeter(timing);
  let bar = 0;
  for (let index = 0; index < points.length; index += 1) {
    const point = points[index];
    const next = points[index + 1];
    if (next === undefined || beat < next.at) return { point, bar };
    bar += Math.round((next.at - point.at) / barLength(point));
  }
  return { point: points[0], bar: 0 };
}

export function barPositionAtBeat(beat, timing) {
  requireBeat(beat, 'beat');
  const segment = meterSegmentAt(beat, timing);
  const length = barLength(segment.point);
  const localBar = Math.floor((beat - segment.point.at) / length + BEAT_EPSILON);
  const barStart = segment.point.at + localBar * length;
  return { bar: segment.bar + localBar, beatInBar: beat - barStart, barStart, barEnd: barStart + length };
}

export function previousBarBoundary(beat, timing) {
  return barPositionAtBeat(beat, timing).barStart;
}

export function nextBarBoundary(beat, timing) {
  const position = barPositionAtBeat(beat, timing);
  return Math.abs(beat - position.barStart) <= BEAT_EPSILON ? position.barStart : position.barEnd;
}

export function isBarBoundary(beat, timing) {
  return Math.abs(beat - previousBarBoundary(beat, timing)) <= BEAT_EPSILON;
}
