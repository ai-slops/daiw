import { RefreshCw } from "lucide-react";
import type { AudioBackend } from "../hooks/use-audio";
import type { NativeDevice } from "../native-audio/bridge";
import { IconButton } from "./primitives";

type AudioBackendControlsProps = Readonly<{
	backend: AudioBackend;
	nativeAvailable: boolean;
	devices: readonly NativeDevice[];
	deviceToken: string | undefined;
	deviceLoading: boolean;
	disabled: boolean;
	onBackend: (backend: AudioBackend) => void;
	onDevice: (token: string) => void;
	onRetry: () => void;
}>;

export const AudioBackendControls = ({
	backend,
	nativeAvailable,
	devices,
	deviceToken,
	deviceLoading,
	disabled,
	onBackend,
	onDevice,
	onRetry,
}: AudioBackendControlsProps) => {
	const supported = devices.filter((device) => device.supported);
	return (
		<div className="audio-backend" data-backend={backend}>
			<label>
				<span>Output</span>
				<select
					aria-label="Playback backend"
					value={backend}
					disabled={disabled}
					onChange={(event) =>
						onBackend(
							event.currentTarget.value === "native" ? "native" : "browser",
						)
					}
				>
					<option value="browser">Browser</option>
					<option value="native" disabled={!nativeAvailable}>
						Native
					</option>
				</select>
			</label>
			{backend === "native" ? (
				<div className="audio-backend__device">
					<label>
						<span>Device</span>
						<select
							aria-label="Native output device"
							value={deviceToken ?? ""}
							disabled={disabled || deviceLoading || supported.length === 0}
							onChange={(event) => onDevice(event.currentTarget.value)}
						>
							{supported.length === 0 ? (
								<option value="">No supported device</option>
							) : (
								supported.map((device) => (
									<option key={device.token} value={device.token}>
										{device.name}
										{device.isDefault ? " · Default" : ""}
									</option>
								))
							)}
						</select>
					</label>
					<IconButton
						label="Retry native output devices"
						icon={RefreshCw}
						disabled={disabled || deviceLoading}
						onClick={onRetry}
					/>
				</div>
			) : null}
			{!nativeAvailable ? (
				<span className="audio-backend__status">
					Native output requires the desktop app.
				</span>
			) : backend === "native" && supported.length === 0 ? (
				<span className="audio-backend__status" role="status">
					Native output unavailable. Connect a supported stereo 48 kHz device,
					then retry.
				</span>
			) : null}
		</div>
	);
};
