import { describe, expect, it } from "vitest";
import {
	AudioBackendSwitcher,
	UnsupportedAudioCapabilityError,
} from "./backend-switch";

describe("AudioBackendSwitcher", () => {
	it("waits for native release before completing the switch to browser", async () => {
		// Given
		let finishRelease: (() => void) | undefined;
		const release = new Promise<void>((resolve) => {
			finishRelease = resolve;
		});
		const switcher = new AudioBackendSwitcher("native");
		let completed = false;
		// When
		const switching = switcher
			.switch("browser", {
				stopBrowser: () => undefined,
				releaseNative: () => release,
			})
			.then(() => {
				completed = true;
			});
		await Promise.resolve();
		// Then
		expect(completed).toBe(false);
		finishRelease?.();
		await switching;
		expect(switcher.backend).toBe("browser");
	});

	it("serializes rapid backend changes without overlapping ownership", async () => {
		// Given
		const events: string[] = [];
		const switcher = new AudioBackendSwitcher("browser");
		const actions = {
			stopBrowser: () => events.push("browser-stopped"),
			releaseNative: async () => {
				events.push("native-released");
			},
		};
		// When
		const native = switcher.switch("native", actions);
		const browser = switcher.switch("browser", actions);
		await Promise.all([native, browser]);
		// Then
		expect(events).toEqual(["browser-stopped", "native-released"]);
		expect(switcher.backend).toBe("browser");
	});

	it("provides a typed error for unsupported native capabilities", () => {
		// Given / When
		const error = new UnsupportedAudioCapabilityError("ab");
		// Then
		expect(error.code).toBe("UnsupportedAudioCapability");
		expect(error).toBeInstanceOf(Error);
	});

	it("keeps native ownership after a failed release and allows a later retry", async () => {
		// Given
		const switcher = new AudioBackendSwitcher("native");
		const stopBrowser = () => undefined;
		// When / Then
		await expect(
			switcher.switch("browser", {
				stopBrowser,
				releaseNative: () => Promise.reject(new Error("release failed")),
			}),
		).rejects.toThrow("release failed");
		expect(switcher.backend).toBe("native");
		await expect(
			switcher.switch("browser", {
				stopBrowser,
				releaseNative: () => Promise.resolve(),
			}),
		).resolves.toBe("browser");
	});
});
