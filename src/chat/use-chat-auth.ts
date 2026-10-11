import { useCallback, useEffect, useState } from "react";
import { completionModeForHostname } from "./auth-window";
import {
	ChatApiError,
	completeLogin,
	getAuthStatus,
	getLoginStatus,
	logoutAccount,
	selectAccount,
	startLogin,
} from "./chat-api";
import type { AuthStatus } from "./chat-contracts";

type PendingLogin = {
	readonly loginId: string;
	readonly authorizationUrl: string;
	readonly redirectUri: string;
	readonly completionMode: "auto" | "paste";
	readonly expiresAt: number;
};

const messageFor = (error: unknown): string => {
	if (error instanceof ChatApiError) {
		if (["LoginExpired", "InvalidLogin"].includes(error.code))
			return `${error.message} Start a new ChatGPT sign-in.`;
		if (["MissingScope", "InsufficientScope"].includes(error.code))
			return `${error.message} Re-authenticate and approve the required ChatGPT permissions.`;
		if (["AccountDisconnected", "Unauthorized"].includes(error.code))
			return `${error.message} Sign in again or choose another account.`;
		if (/usage|limit|quota|rate/i.test(error.code))
			return `${error.message} Check account usage or choose another model.`;
		return error.message;
	}
	if (error instanceof Error) return error.message;
	return "Chat account request failed.";
};

export const useChatAuth = () => {
	const [status, setStatus] = useState<AuthStatus>();
	const [pending, setPending] = useState<PendingLogin>();
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string>();

	const refresh = useCallback(async (): Promise<AuthStatus | undefined> => {
		try {
			const next = await getAuthStatus();
			setStatus(next);
			setError(undefined);
			return next;
		} catch (cause) {
			setError(messageFor(cause));
			return undefined;
		}
	}, []);

	useEffect(() => {
		void refresh();
	}, [refresh]);

	useEffect(() => {
		if (pending === undefined || pending.completionMode !== "auto") return;
		let active = true;
		let timeout: number | undefined;
		const poll = async () => {
			if (!active) return;
			try {
				const next = await getLoginStatus(pending.loginId);
				if (!active) return;
				switch (next.status) {
					case "pending":
						timeout = window.setTimeout(() => void poll(), 2_000);
						return;
					case "completed":
						await refresh();
						if (active) setPending(undefined);
						return;
					case "failed":
						setError(
							messageFor(new ChatApiError(next.error.code, next.error.message)),
						);
						setPending(undefined);
						return;
					case "expired":
						setError(
							next.error?.message ??
								"Sign-in expired. Start a new ChatGPT sign-in.",
						);
						setPending(undefined);
						return;
					default:
						return next satisfies never;
				}
			} catch (cause) {
				if (active) setError(messageFor(cause));
			}
		};
		timeout = window.setTimeout(() => void poll(), 2_000);
		return () => {
			active = false;
			if (timeout !== undefined) window.clearTimeout(timeout);
		};
	}, [pending, refresh]);

	const login = useCallback(
		async (accountId?: string, mode?: "auto" | "paste") => {
			setBusy(true);
			setError(undefined);
			try {
				const value = await startLogin(
					accountId,
					mode ?? completionModeForHostname(window.location.hostname),
				);
				setPending(value);
				return value.authorizationUrl;
			} catch (cause) {
				setError(messageFor(cause));
				return undefined;
			} finally {
				setBusy(false);
			}
		},
		[],
	);

	const finishPasteLogin = useCallback(
		async (callbackUrl: string) => {
			if (pending === undefined) return;
			setBusy(true);
			setError(undefined);
			try {
				await completeLogin(pending.loginId, callbackUrl);
				await refresh();
				setPending(undefined);
			} catch (cause) {
				setError(messageFor(cause));
			} finally {
				setBusy(false);
			}
		},
		[pending, refresh],
	);

	const chooseAccount = useCallback(
		async (accountId: string) => {
			setBusy(true);
			try {
				await selectAccount(accountId);
				await refresh();
			} catch (cause) {
				setError(messageFor(cause));
			} finally {
				setBusy(false);
			}
		},
		[refresh],
	);

	const logout = useCallback(
		async (accountId: string) => {
			setBusy(true);
			try {
				const result = await logoutAccount(accountId);
				await refresh();
				if (!result.revocationConfirmed)
					setError(
						"Signed out locally. OpenAI could not confirm token revocation.",
					);
			} catch (cause) {
				setError(messageFor(cause));
			} finally {
				setBusy(false);
			}
		},
		[refresh],
	);

	return {
		status,
		pending,
		busy,
		error,
		refresh,
		login,
		finishPasteLogin,
		chooseAccount,
		logout,
	};
};
