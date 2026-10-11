import { ExternalLink, LogOut, RefreshCw, UserRoundPlus } from "lucide-react";
import { useState } from "react";
import {
	createCallbackUrlSchema,
	navigateAuthWindow,
	openPendingAuthWindow,
} from "../../chat/auth-window";
import { Button } from "../primitives";
import type { ReturnTypeOfChatAuth } from "./chat-types";

type ChatAccountControlsProps = {
	readonly auth: ReturnTypeOfChatAuth;
};

export const ChatAccountControls = ({ auth }: ChatAccountControlsProps) => {
	const [callbackUrl, setCallbackUrl] = useState("");
	const [authorizationUrl, setAuthorizationUrl] = useState<string>();
	const [callbackError, setCallbackError] = useState<string>();
	const [navigationError, setNavigationError] = useState<string>();
	const [selectedAccountId, setSelectedAccountId] = useState("");
	const activeId = auth.status?.activeAccountId ?? null;
	const accounts = auth.status?.accounts ?? [];
	const selectedId =
		activeId ??
		(accounts.some((account) => account.id === selectedAccountId)
			? selectedAccountId
			: "");
	const selected = accounts.find((account) => account.id === selectedId);
	const beginLogin = async (accountId?: string) => {
		setNavigationError(undefined);
		const opened = openPendingAuthWindow();
		const url = await auth.login(accountId);
		if (url === undefined) {
			opened?.close();
			return;
		}
		setAuthorizationUrl(url);
		try {
			const navigated = await navigateAuthWindow(url, opened);
			if (!navigated)
				setNavigationError(
					"Could not open ChatGPT sign-in. Use the sign-in link below.",
				);
		} catch {
			opened?.close();
			setNavigationError(
				"Could not open ChatGPT sign-in. Use the sign-in link below.",
			);
		}
	};

	return (
		<section className="chat-account" aria-label="ChatGPT account">
			<div className="chat-account__status">
				<span
					className="status-dot"
					data-connected={selected?.connected === true}
				/>
				<div>
					<strong>{selected?.profile.label ?? "No ChatGPT account"}</strong>
					<small>
						{selected?.connected === true
							? "Connected"
							: "Sign in to start a chat"}
					</small>
				</div>
			</div>
			{auth.status === undefined || auth.status.accounts.length === 0 ? (
				<Button
					variant="primary"
					disabled={auth.busy}
					onClick={() => void beginLogin()}
				>
					Continue with ChatGPT
				</Button>
			) : (
				<>
					<label className="chat-field">
						<span>Account</span>
						<select
							value={selectedId}
							onChange={(event) => {
								const accountId = event.currentTarget.value;
								setSelectedAccountId(accountId);
								const account = accounts.find(
									(candidate) => candidate.id === accountId,
								);
								if (account?.connected === true)
									void auth.chooseAccount(accountId);
							}}
						>
							<option value="" disabled>
								Select an account
							</option>
							{auth.status.accounts.map((account) => (
								<option key={account.id} value={account.id}>
									{account.profile.label}
								</option>
							))}
						</select>
					</label>
					<div className="cluster">
						<Button
							icon={UserRoundPlus}
							variant="ghost"
							disabled={auth.busy}
							onClick={() => void beginLogin()}
						>
							New account
						</Button>
						{selectedId.length === 0 ? null : (
							<Button
								variant="ghost"
								disabled={auth.busy}
								onClick={() => void beginLogin(selectedId)}
							>
								Re-authenticate
							</Button>
						)}
						{selectedId.length === 0 ? null : (
							<Button
								icon={LogOut}
								variant="ghost"
								disabled={auth.busy}
								onClick={() => void auth.logout(selectedId)}
							>
								Log out
							</Button>
						)}
					</div>
				</>
			)}
			<Button
				icon={RefreshCw}
				variant="ghost"
				disabled={auth.busy}
				onClick={() => void auth.refresh()}
			>
				Refresh status
			</Button>
			{authorizationUrl === undefined ? null : (
				<a
					className="chat-auth-link"
					href={authorizationUrl}
					target="_blank"
					rel="noreferrer"
				>
					<ExternalLink aria-hidden="true" />
					Open ChatGPT sign-in
				</a>
			)}
			{navigationError === undefined ? null : (
				<p className="chat-inline-error" role="alert">
					{navigationError}
				</p>
			)}
			{auth.pending?.completionMode !== "paste" ? null : (
				<form
					className="chat-paste"
					onSubmit={(event) => {
						event.preventDefault();
						const pending = auth.pending;
						if (pending === undefined) return;
						const parsed = createCallbackUrlSchema(
							pending.redirectUri,
							pending.authorizationUrl,
						).safeParse(callbackUrl.trim());
						if (!parsed.success) {
							setCallbackError(
								parsed.error.issues[0]?.message ?? "Invalid callback URL.",
							);
							return;
						}
						setCallbackError(undefined);
						void auth.finishPasteLogin(parsed.data);
					}}
				>
					<p>Finish this sign-in manually:</p>
					<ol>
						<li>Open ChatGPT sign-in and approve access.</li>
						<li>Copy the final 127.0.0.1 callback URL.</li>
						<li>Paste it below to complete this login attempt.</li>
					</ol>
					<input
						aria-label="OAuth callback URL"
						value={callbackUrl}
						onChange={(event) => setCallbackUrl(event.currentTarget.value)}
						placeholder="http://127.0.0.1:…"
					/>
					{callbackError === undefined ? null : (
						<small role="alert">{callbackError}</small>
					)}
					<Button
						variant="primary"
						disabled={auth.busy || callbackUrl.trim().length === 0}
						type="submit"
					>
						Complete sign-in
					</Button>
				</form>
			)}
			{auth.error === undefined ? null : (
				<p className="chat-inline-error" role="alert">
					{auth.error}
				</p>
			)}
		</section>
	);
};
