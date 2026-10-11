import { execFile } from "node:child_process";
import { randomBytes, randomUUID } from "node:crypto";
import { constants } from "node:fs";
import { access } from "node:fs/promises";
import { promisify } from "node:util";
import { z } from "zod";
import { NativeAudioError } from "./native-audio-worker.mjs";

const exec = promisify(execFile);
const devicesSchema = z
	.object({
		v: z.literal(1),
		devices: z.array(
			z
				.object({
					index: z.number().int().nonnegative(),
					name: z.string(),
					nameSha256: z.string().regex(/^[a-f0-9]{64}$/),
					isDefault: z.boolean(),
					supported: z.boolean(),
				})
				.strict(),
		),
	})
	.strict();
const token = () => randomBytes(24).toString("base64url");

export class NativeDeviceRegistry {
	#binary;
	#lists = new Map();

	constructor(binary) {
		this.#binary = binary;
	}

	async capabilities() {
		let available = true;
		try {
			await access(this.#binary, constants.X_OK);
		} catch {
			available = false;
		}
		return {
			available,
			sampleRate: 48_000,
			channels: 2,
			ab: false,
			meter: false,
		};
	}

	async list(clientId) {
		let stdout;
		try {
			({ stdout } = await exec(
				this.#binary,
				["--list-output-devices", "--json"],
				{ encoding: "utf8", maxBuffer: 64 * 1024, timeout: 5000 },
			));
		} catch {
			throw new NativeAudioError(
				"NativeUnavailable",
				"native audio player is unavailable",
			);
		}
		let listed;
		try {
			listed = devicesSchema.parse(JSON.parse(stdout));
		} catch {
			throw new NativeAudioError(
				"InvalidProtocol",
				"native audio device response was invalid",
			);
		}
		const generation = randomUUID();
		const devices = listed.devices.map((device) => ({
			token: token(),
			name: device.name,
			isDefault: device.isDefault,
			supported: device.supported,
			selector: { index: device.index, digest: device.nameSha256 },
		}));
		this.#lists.set(clientId, { generation, devices });
		return {
			generation,
			devices: devices.map(({ selector: _selector, ...device }) => device),
		};
	}

	selector(clientId, deviceToken) {
		const device = this.#lists
			.get(clientId)
			?.devices.find((candidate) => candidate.token === deviceToken);
		if (device === undefined || !device.supported)
			throw new NativeAudioError(
				"InvalidDevice",
				"output device token is stale or invalid",
			);
		return device.selector;
	}

	releaseClient(clientId) {
		this.#lists.delete(clientId);
	}

	clear() {
		this.#lists.clear();
	}
}
