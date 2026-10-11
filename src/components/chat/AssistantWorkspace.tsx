import { Bot, CircleAlert, Link2, MessageSquareText, Plus } from "lucide-react";
import { useState } from "react";
import { useChatAuth } from "../../chat/use-chat-auth";
import { useChatSession } from "../../chat/use-chat-session";
import { ProposalPanel, type ProposalPanelProps } from "../ProposalPanel";
import { Button, Panel, Tabs } from "../primitives";
import { ChatAccountControls } from "./ChatAccountControls";
import { ChatThread } from "./ChatThread";

type AssistantTab = "chat" | "review";
const tabs = [
	{ value: "chat", label: "Agent chat" },
	{ value: "review", label: "Proposal review" },
] as const;

type AssistantWorkspaceProps = ProposalPanelProps & {
	readonly projectId: string;
	readonly projectName: string;
	readonly onProjectChanged: (projectId: string) => void;
};

type SendEligibility = {
	readonly approved: boolean;
	readonly loading: boolean;
	readonly projectId: string;
	readonly activeAccountId: string | null;
	readonly activeAccountConnected: boolean;
	readonly scope: {
		readonly projectId: string;
		readonly accountId: string;
	} | null;
};

export const canSendChat = ({
	approved,
	loading,
	projectId,
	activeAccountId,
	activeAccountConnected,
	scope,
}: SendEligibility): boolean =>
	approved &&
	!loading &&
	activeAccountConnected &&
	activeAccountId !== null &&
	scope?.projectId === projectId &&
	scope.accountId === activeAccountId;

export const AssistantWorkspace = ({
	projectId,
	projectName,
	onProjectChanged,
	...proposalProps
}: AssistantWorkspaceProps) => {
	const [tab, setTab] = useState<AssistantTab>("chat");
	const auth = useChatAuth();
	const activeAccountId = auth.status?.activeAccountId ?? null;
	const activeAccount = auth.status?.accounts.find(
		(account) => account.id === activeAccountId,
	);
	const [contextApproval, setContextApproval] = useState({
		projectId,
		accountId: activeAccountId,
		approved: false,
	});
	const contextApproved =
		contextApproval.projectId === projectId &&
		contextApproval.accountId === activeAccountId &&
		contextApproval.approved;
	const chat = useChatSession({
		projectId,
		accountId: activeAccountId,
		onProjectChanged,
	});
	const running = chat.state.turns.some((turn) => turn.status === "running");
	const canSend = canSendChat({
		approved: contextApproved,
		loading: chat.loading,
		projectId,
		activeAccountId,
		activeAccountConnected: activeAccount?.connected === true,
		scope: chat.state.scope,
	});

	if (tab === "review") {
		return (
			<div className="assistant-workspace">
				<div className="assistant-workspace__tabs">
					<Tabs
						value={tab}
						items={tabs}
						onChange={setTab}
						label="Assistant workspace"
					/>
				</div>
				<ProposalPanel {...proposalProps} />
			</div>
		);
	}

	return (
		<Panel
			title="Assistant"
			eyebrow="ChatGPT · project agent"
			className="assistant-chat-panel"
			actions={
				<span className="chat-live-state" data-connected={chat.connected}>
					{chat.connected ? "Live" : running ? "Reconnecting" : "Ready"}
				</span>
			}
		>
			<div className="assistant-chat-layout">
				<aside className="chat-sidebar">
					<Tabs
						value={tab}
						items={tabs}
						onChange={setTab}
						label="Assistant workspace"
					/>
					<ChatAccountControls auth={auth} />
					{auth.status?.activeAccountId === null ||
					auth.status?.activeAccountId === undefined ? null : (
						<section
							className="chat-session-controls"
							aria-label="Chat sessions"
						>
							<label className="chat-field">
								<span>Model</span>
								<select
									value={chat.model}
									disabled={chat.loading || chat.models.length === 0}
									onChange={(event) => chat.setModel(event.currentTarget.value)}
								>
									{chat.models.map((model) => (
										<option key={model.id} value={model.id}>
											{model.displayName}
										</option>
									))}
								</select>
							</label>
							<Button
								icon={Plus}
								variant="primary"
								disabled={chat.loading || chat.model.length === 0}
								onClick={() => void chat.startNew()}
							>
								New chat
							</Button>
							<div className="chat-session-list">
								<span>Recent sessions</span>
								{chat.sessions.length === 0 ? (
									<small>No saved sessions for this project.</small>
								) : (
									chat.sessions.map((session) => (
										<button
											type="button"
											key={session.id}
											data-active={session.id === chat.state.sessionId}
											onClick={() => void chat.open(session)}
										>
											<strong>{session.title || "Untitled chat"}</strong>
											<small>{session.model}</small>
										</button>
									))
								)}
							</div>
						</section>
					)}
				</aside>
				<section className="chat-stage" aria-label="Project agent chat">
					<header className="chat-context-bar">
						<div>
							<Bot aria-hidden="true" />
							<span>
								<strong>{projectName}</strong>
								<small>Project agent</small>
							</span>
						</div>
						{proposalProps.instance === undefined ? (
							<span className="chat-context-chip">No clip selected</span>
						) : (
							<span className="chat-context-chip">
								<Link2 aria-hidden="true" />
								{proposalProps.instance.name}
								<small>{proposalProps.instance.id}</small>
							</span>
						)}
					</header>
					<label className="chat-consent">
						<input
							type="checkbox"
							checked={contextApproved}
							onChange={(event) =>
								setContextApproval({
									projectId,
									accountId: activeAccountId,
									approved: event.currentTarget.checked,
								})
							}
						/>
						<span>
							<strong>Send this project context to OpenAI when I send.</strong>
							<small>
								Structure and selected clip references may be included. Raw
								audio is not uploaded.
							</small>
						</span>
					</label>
					{chat.state.sessionId === null ? (
						<div className="chat-start-state">
							<MessageSquareText aria-hidden="true" />
							<strong>Start or open a chat</strong>
							<span>
								General project questions work without selecting a clip.
							</span>
						</div>
					) : (
						<ChatThread
							turns={chat.state.turns}
							canSend={canSend}
							onSend={chat.send}
							onCancel={chat.cancel}
						/>
					)}
					{chat.error === undefined ? null : (
						<div className="chat-error" role="alert">
							<CircleAlert aria-hidden="true" />
							<span>{chat.error}</span>
							{running ? (
								<Button variant="ghost" onClick={chat.reconnect}>
									Reconnect
								</Button>
							) : null}
						</div>
					)}
				</section>
			</div>
		</Panel>
	);
};
