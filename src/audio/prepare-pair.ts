import { AudioEngineError } from "./errors";
import type { StreamBranch } from "./stream-branch";

export async function settlePreparedPair(
	first: Promise<StreamBranch>,
	second: Promise<StreamBranch>,
	cancel: () => void,
): Promise<readonly [StreamBranch, StreamBranch]> {
	let failed = false;
	let firstFailure: unknown;
	const observe = async (
		preparation: Promise<StreamBranch>,
	): Promise<StreamBranch> => {
		try {
			return await preparation;
		} catch (reason) {
			if (!failed) {
				failed = true;
				firstFailure = reason;
				cancel();
			}
			throw reason;
		}
	};
	const [firstResult, secondResult] = await Promise.allSettled([
		observe(first),
		observe(second),
	]);
	if (
		firstResult.status === "fulfilled" &&
		secondResult.status === "fulfilled"
	) {
		return [firstResult.value, secondResult.value];
	}
	if (firstResult.status === "fulfilled") firstResult.value.clear();
	if (secondResult.status === "fulfilled") secondResult.value.clear();
	if (firstFailure instanceof Error) throw firstFailure;
	throw new AudioEngineError("A/B preparation failed");
}

type PairPrimePlan = Readonly<{
	frame: number;
	signal: AbortSignal;
	cancel: () => void;
}>;

export function primePreparedPair(
	pair: readonly [StreamBranch, StreamBranch],
	plan: PairPrimePlan,
): Promise<readonly [StreamBranch, StreamBranch]> {
	const [first, second] = pair;
	return settlePreparedPair(
		first.prime(plan.frame, plan.signal).then(() => first),
		second.prime(plan.frame, plan.signal).then(() => second),
		plan.cancel,
	);
}
