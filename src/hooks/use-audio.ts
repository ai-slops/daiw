import { useCallback, useRef, useState } from "react";
import {
	type AudioBackend,
	AudioBackendReleaseError,
	AudioBackendSwitcher,
	UnsupportedAudioCapabilityError,
} from "../audio/backend-switch";
import type { Project } from "../domain/project";
import { useBrowserAudio } from "./use-browser-audio";
import { useNativeAudio } from "./use-native-audio";

export type { AudioBackend } from "../audio/backend-switch";

export const useAudio = (project: Project) => {
	const [backend, setBackendState] = useState<AudioBackend>("browser");
	const [transitioning, setTransitioning] = useState(false);
	const switcher = useRef(new AudioBackendSwitcher("browser"));
	const transition = useRef(0);
	const browser = useBrowserAudio(project);
	const native = useNativeAudio(project, backend === "native");
	const active = backend === "browser" ? browser : native;
	const setBackend = useCallback(
		async (next: AudioBackend) => {
			const operation = transition.current + 1;
			transition.current = operation;
			setTransitioning(true);
			try {
				const activeBackend = await switcher.current.switch(next, {
					stopBrowser: browser.stop,
					releaseNative: async () => {
						if (!(await native.release())) throw new AudioBackendReleaseError();
					},
				});
				setBackendState(activeBackend);
			} catch (reason) {
				if (!(reason instanceof AudioBackendReleaseError)) throw reason;
			} finally {
				if (transition.current === operation) setTransitioning(false);
			}
		},
		[browser.stop, native.release],
	);
	return {
		...active,
		busy: active.busy || transitioning,
		backend,
		nativeAvailable: native.available,
		devices: native.devices,
		deviceToken: native.deviceToken,
		deviceLoading: native.deviceLoading,
		refreshDevices: native.refreshDevices,
		setDeviceToken: native.setDeviceToken,
		supportsAB: backend === "browser",
		supportsMeter: backend === "browser",
		setBackend,
		prepareAB:
			backend === "browser"
				? browser.prepareAB
				: async () => {
						throw new UnsupportedAudioCapabilityError("ab");
					},
		selectAB:
			backend === "browser"
				? browser.selectAB
				: () => {
						throw new UnsupportedAudioCapabilityError("ab");
					},
		setLoudnessMatching:
			backend === "browser"
				? browser.setLoudnessMatching
				: () => {
						throw new UnsupportedAudioCapabilityError("loudnessMatching");
					},
	};
};
