import { describe, expect, it } from "vitest";
import { loudnessMatchGain, measureBufferRms, meterFromSamples } from "./analysis";

function buffer(channels: readonly Float32Array[]): {
  readonly numberOfChannels: number;
  getChannelData(channel: number): Float32Array;
} {
  return {
    numberOfChannels: channels.length,
    getChannelData(channel) {
      const data = channels[channel];
      if (data === undefined) throw new RangeError("channel is out of range");
      return data;
    },
  };
}

describe("A/B loudness analysis", () => {
  it("measures all channels without collapsing stereo", () => {
    // Given
    const audio = buffer([new Float32Array([1, 1]), new Float32Array([0, 0])]);

    // When
    const rms = measureBufferRms(audio);

    // Then
    expect(rms).toBeCloseTo(Math.sqrt(0.5));
  });

  it("matches candidate RMS to the reference within a bounded gain", () => {
    // Given
    const reference = buffer([new Float32Array([0.5, -0.5])]);
    const candidate = buffer([new Float32Array([0.25, -0.25])]);

    // When
    const gain = loudnessMatchGain(reference, candidate);

    // Then
    expect(gain).toBeCloseTo(2);
  });
});

describe("master meter", () => {
  it("returns negative infinity dB for silence", () => {
    // Given
    const samples = new Float32Array(32);

    // When
    const meter = meterFromSamples(samples);

    // Then
    expect(meter).toEqual({ peak: 0, rms: 0, peakDb: -Infinity, rmsDb: -Infinity });
  });

  it("returns peak and RMS from the current signal window", () => {
    // Given
    const samples = new Float32Array([1, -1, 0, 0]);

    // When
    const meter = meterFromSamples(samples);

    // Then
    expect(meter.peak).toBe(1);
    expect(meter.rms).toBeCloseTo(Math.sqrt(0.5));
    expect(meter.peakDb).toBeCloseTo(0);
  });
});
