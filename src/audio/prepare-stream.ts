import { AudioEngineError } from "./errors";
import { openServerRender, type ServerRenderRequest } from "./server-render";
import { StreamBranch } from "./stream-branch";
import type { StreamBufferBudget } from "./stream-budget";

type Preparation = Readonly<{
	frame: number;
	expectedFrames: number;
	signal: AbortSignal;
	budget: StreamBufferBudget;
}>;

export async function prepareStream(
	context: AudioContext,
	request: ServerRenderRequest,
	preparation: Preparation,
): Promise<StreamBranch> {
	const descriptor = await openServerRender(request, preparation.signal);
	if (descriptor.info.frames !== preparation.expectedFrames) {
		throw new AudioEngineError(
			`render frame mismatch: expected ${preparation.expectedFrames}, received ${descriptor.info.frames}`,
		);
	}
	const stream = new StreamBranch(context, descriptor, preparation.budget);
	try {
		await stream.prime(preparation.frame, preparation.signal);
		return stream;
	} catch (reason) {
		stream.clear();
		throw reason;
	}
}
