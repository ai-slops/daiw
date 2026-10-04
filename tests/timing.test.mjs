import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  MAX_BEATS,
  barPositionAtBeat,
  beatAtSeconds,
  isBarBoundary,
  nextBarBoundary,
  previousBarBoundary,
  secondsAtBeat,
  validateMeterMap,
  validateTempoMap,
} from '../shared/timing.mjs';

const stepTempo = {
  points: [
    { at: 0, bpm: 120, curve: 'step' },
    { at: 4, bpm: 60, curve: 'step' },
  ],
};

const rampTempo = {
  points: [
    { at: 0, bpm: 120, curve: 'linear' },
    { at: 4, bpm: 240, curve: 'step' },
  ],
};

test('tempo maps integrate golden step and linear vectors', () => {
  assert.equal(secondsAtBeat(6, stepTempo), 4);
  assert.ok(Math.abs(secondsAtBeat(4, rampTempo) - 2 * Math.log(2)) < 1e-12);
  assert.ok(Math.abs(secondsAtBeat(6, rampTempo) - (2 * Math.log(2) + 0.5)) < 1e-12);
});

test('tempo-map inversion round-trips boundaries and ramp interiors', () => {
  for (const beat of [0, 1.25, 4, 9.5]) {
    assert.ok(Math.abs(beatAtSeconds(secondsAtBeat(beat, rampTempo), rampTempo) - beat) < 1e-10);
  }
});

test('equal linear endpoints use the stable constant-tempo case', () => {
  const equalRamp = { points: [{ at: 0, bpm: 120, curve: 'linear' }, { at: 8, bpm: 120, curve: 'step' }] };
  assert.equal(secondsAtBeat(8, equalRamp), 4);
  assert.equal(beatAtSeconds(4, equalRamp), 8);
});

test('meter utilities follow map changes without changing absolute time', () => {
  const meterMap = { points: [{ at: 0, numerator: 4, denominator: 4 }, { at: 8, numerator: 3, denominator: 4 }] };
  assert.deepEqual(barPositionAtBeat(9, meterMap), { bar: 2, beatInBar: 1, barStart: 8, barEnd: 11 });
  assert.equal(previousBarBoundary(10.5, meterMap), 8);
  assert.equal(nextBarBoundary(10.5, meterMap), 11);
  assert.equal(isBarBoundary(11, meterMap), true);
  assert.equal(secondsAtBeat(9, 120), 4.5);
});

test('map validators reject invalid boundaries and negative utility inputs', () => {
  assert.throws(() => validateTempoMap({ points: [{ at: 1, bpm: 120, curve: 'step' }] }), RangeError);
  assert.throws(() => validateTempoMap({ points: [{ at: 0, bpm: 120, curve: 'linear' }] }), RangeError);
  assert.throws(() => validateTempoMap({ points: [{ at: 0, bpm: 19, curve: 'step' }] }), RangeError);
  assert.throws(() => validateMeterMap({ points: [{ at: 0, numerator: 4, denominator: 4 }, { at: 3, numerator: 3, denominator: 4 }] }), RangeError);
  assert.throws(() => secondsAtBeat(-1, 120), RangeError);
  assert.throws(() => beatAtSeconds(-1, 120), RangeError);
});

test('validators match core tick rounding and maximum beat bounds', () => {
  assert.equal(MAX_BEATS, 1_000_000);
  assert.doesNotThrow(() => validateTempoMap({ points: [
    { at: 0, bpm: 120, curve: 'step' },
    { at: 16385.739583333332, bpm: 100, curve: 'step' },
  ] }));
  assert.doesNotThrow(() => validateMeterMap({ points: [
    { at: 0, numerator: 1, denominator: 49 },
    { at: 1_000_000, numerator: 4, denominator: 4 },
  ] }));
  assert.throws(() => validateTempoMap({ points: [{ at: 1_000_001, bpm: 120, curve: 'step' }] }), RangeError);
});
