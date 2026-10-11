import { useCallback, useEffect, useState } from "react";
import { selectNativeDevice } from "../audio/native-adapter";
import type { NativeDevice, nativeBridge } from "../native-audio/bridge";

type NativeBridge = NonNullable<ReturnType<typeof nativeBridge>>;

export const useNativeDevices = (bridge: NativeBridge | undefined) => {
	const [devices, setDevices] = useState<readonly NativeDevice[]>([]);
	const [deviceToken, setDeviceToken] = useState<string>();
	const [deviceLoading, setDeviceLoading] = useState(false);
	const [available, setAvailable] = useState(false);
	const [error, setError] = useState<string>();

	const refreshDevices = useCallback(async () => {
		if (bridge === undefined) return;
		setDeviceLoading(true);
		setError(undefined);
		try {
			const capabilities = await bridge.capabilities();
			setAvailable(capabilities.available);
			if (!capabilities.available) {
				setDevices([]);
				setDeviceToken(undefined);
				setError(
					"Native audio is unavailable. Build the desktop audio player, then retry.",
				);
				return;
			}
			const listed = await bridge.listDevices();
			setDevices(listed.devices);
			setDeviceToken((current) => selectNativeDevice(current, listed.devices));
		} catch (reason) {
			setAvailable(false);
			setDevices([]);
			setDeviceToken(undefined);
			setError(
				reason instanceof Error
					? reason.message
					: "Could not enumerate native output devices.",
			);
		} finally {
			setDeviceLoading(false);
		}
	}, [bridge]);

	useEffect(() => {
		if (bridge !== undefined) void refreshDevices();
	}, [bridge, refreshDevices]);

	return {
		available,
		devices,
		deviceToken,
		deviceLoading,
		error,
		setError,
		setDeviceToken,
		refreshDevices,
	};
};
