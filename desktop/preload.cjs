const { contextBridge, ipcRenderer } = require("electron");

const invoke = (channel, payload = {}) => ipcRenderer.invoke(channel, payload);
contextBridge.exposeInMainWorld(
	"daiwNativeAudio",
	Object.freeze({
		capabilities: () => invoke("daiw-native:capabilities"),
		listDevices: () => invoke("daiw-native:list-devices"),
		prepare: (request) => invoke("daiw-native:prepare", request),
		play: (sessionToken) => invoke("daiw-native:play", { sessionToken }),
		pause: (sessionToken) => invoke("daiw-native:pause", { sessionToken }),
		stop: (sessionToken) => invoke("daiw-native:stop", { sessionToken }),
		seek: (sessionToken, frame) =>
			invoke("daiw-native:seek", { sessionToken, frame }),
		setLoop: (sessionToken, loop) =>
			invoke("daiw-native:set-loop", { sessionToken, loop }),
		release: (sessionToken) => invoke("daiw-native:release", { sessionToken }),
		state: (sessionToken) => invoke("daiw-native:state", { sessionToken }),
		onState: (listener) => {
			const handler = (_event, state) => listener(state);
			ipcRenderer.on("daiw-native:state-changed", handler);
			return () =>
				ipcRenderer.removeListener("daiw-native:state-changed", handler);
		},
	}),
);
