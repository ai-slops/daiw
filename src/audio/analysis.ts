export type AudioPcmBuffer = Readonly<{
	numberOfChannels: number;
	getChannelData(channel: number): Float32Array;
}>;

export function measureBufferRms(buffer: AudioPcmBuffer): number {
	let squareSum = 0;
	let sampleCount = 0;
	for (let channel = 0; channel < buffer.numberOfChannels; channel += 1) {
		const samples = buffer.getChannelData(channel);
		for (const sample of samples) squareSum += sample * sample;
		sampleCount += samples.length;
	}
	return sampleCount === 0 ? 0 : Math.sqrt(squareSum / sampleCount);
}

export function loudnessMatchGain(
	reference: AudioPcmBuffer,
	candidate: AudioPcmBuffer,
): number {
	const referenceRms = measureBufferRms(reference);
	const candidateRms = measureBufferRms(candidate);
	if (referenceRms === 0 || candidateRms === 0) return 1;
	return Math.max(0.25, Math.min(4, referenceRms / candidateRms));
}

type MeterAccumulator = {
	peak: number;
	rms: number;
	peakDb: number;
	rmsDb: number;
};

export function meterFromSamples(samples: Float32Array): MasterMeter {
	return updateMeter(samples, {
		peak: 0,
		rms: 0,
		peakDb: -Infinity,
		rmsDb: -Infinity,
	});
}

export function updateMeter(
	samples: Float32Array,
	output: MeterAccumulator,
): MasterMeter {
	let peak = 0;
	let squareSum = 0;
	for (const sample of samples) {
		peak = Math.max(peak, Math.abs(sample));
		squareSum += sample * sample;
	}
	const rms = samples.length === 0 ? 0 : Math.sqrt(squareSum / samples.length);
	output.peak = peak;
	output.rms = rms;
	output.peakDb = peak === 0 ? -Infinity : 20 * Math.log10(peak);
	output.rmsDb = rms === 0 ? -Infinity : 20 * Math.log10(rms);
	return output;
}

import type { MasterMeter } from "./types";
