import {
	type AppendMessage,
	AssistantRuntimeProvider,
	ComposerPrimitive,
	MessagePrimitive,
	type ThreadMessageLike,
	ThreadPrimitive,
	type ToolCallMessagePartProps,
	useExternalStoreRuntime,
} from "@assistant-ui/react";
import { CircleAlert, Send, Square } from "lucide-react";
import { useMemo } from "react";
import type { ChatTool, ChatTurn } from "../../chat/chat-contracts";
import { ToolNameSchema } from "../../chat/chat-contracts";

export type ChatRow = {
	readonly id: string;
	readonly role: "user" | "assistant";
	readonly text: string;
	readonly status?: ChatTurn["status"];
	readonly tools: readonly ChatTool[];
};

type TurnError = NonNullable<ChatTurn["error"]>;

const recoveryForError = (code: string): string => {
	if (/auth|login|scope|token|expired|disconnected/i.test(code))
		return "Sign in again or choose another account.";
	if (/limit|quota|usage|rate/i.test(code))
		return "Check account usage or choose another model.";
	return "Try again or start a new chat.";
};

type TurnErrorAlertProps = {
	readonly error: TurnError;
	readonly canRetry: boolean;
	readonly onRetry: () => Promise<void>;
};

export const TurnErrorAlert = ({
	error,
	canRetry,
	onRetry,
}: TurnErrorAlertProps) => (
	<div className="chat-turn-error" role="alert">
		<CircleAlert aria-hidden="true" />
		<div>
			<strong>{error.code}</strong>
			<span>{error.message}</span>
			<small>{recoveryForError(error.code)}</small>
		</div>
		<button type="button" disabled={!canRetry} onClick={() => void onRetry()}>
			Try again
		</button>
	</div>
);

const statusFor = (
	status: ChatTurn["status"] | undefined,
): ThreadMessageLike["status"] => {
	switch (status) {
		case "running":
			return { type: "running" };
		case "completed":
			return { type: "complete", reason: "stop" };
		case "cancelled":
			return { type: "incomplete", reason: "cancelled" };
		case "failed":
		case "interrupted":
			return { type: "incomplete", reason: "error" };
		case undefined:
			return undefined;
		default:
			return status satisfies never;
	}
};

export const convertRow = (row: ChatRow): ThreadMessageLike => ({
	id: row.id,
	role: row.role,
	content:
		row.role === "user"
			? [{ type: "text", text: row.text }]
			: [
					{ type: "text", text: row.text },
					...row.tools.map((tool) => ({
						type: "tool-call" as const,
						toolCallId: tool.callId,
						toolName: tool.name,
						args: {},
						isError: tool.status === "failed",
						...(tool.result === undefined ? {} : { result: tool.result }),
					})),
				],
	...(row.role === "assistant" ? { status: statusFor(row.status) } : {}),
});

const messageText = (message: AppendMessage): string =>
	message.content
		.filter((part) => part.type === "text")
		.map((part) => part.text)
		.join("\n")
		.trim();

const ToolCard = ({
	toolName,
	result,
	isError,
}: ToolCallMessagePartProps<Readonly<Record<string, unknown>>, unknown>) => {
	const parsedName = ToolNameSchema.safeParse(toolName);
	const label = parsedName.success
		? parsedName.data.replaceAll("_", " ")
		: "unsupported tool";
	return (
		<section
			className="chat-tool"
			data-state={
				isError ? "failed" : result === undefined ? "running" : "completed"
			}
		>
			<span>
				{result === undefined ? "Working" : isError ? "Failed" : "Completed"}
			</span>
			<strong>{label}</strong>
			{result === undefined ? null : (
				<small>Result attached to this session</small>
			)}
		</section>
	);
};

const ChatMessage = () => (
	<MessagePrimitive.Root className="chat-message">
		<MessagePrimitive.Parts components={{ tools: { Fallback: ToolCard } }} />
	</MessagePrimitive.Root>
);

type ChatThreadProps = {
	readonly turns: readonly ChatTurn[];
	readonly canSend: boolean;
	readonly onSend: (text: string) => Promise<void>;
	readonly onCancel: () => Promise<void>;
};

export const ChatThread = ({
	turns,
	canSend,
	onSend,
	onCancel,
}: ChatThreadProps) => {
	const rows = useMemo<readonly ChatRow[]>(
		() =>
			turns.flatMap((turn) => [
				{ id: `${turn.id}-user`, role: "user", text: turn.text, tools: [] },
				{
					id: `${turn.id}-assistant`,
					role: "assistant",
					text: turn.assistantText,
					status: turn.status,
					tools: turn.tools,
				},
			]),
		[turns],
	);
	const running = turns.some((turn) => turn.status === "running");
	const runtime = useExternalStoreRuntime<ChatRow>({
		messages: rows,
		convertMessage: convertRow,
		isRunning: running,
		isSendDisabled: !canSend,
		onNew: async (message) => {
			const text = messageText(message);
			if (text.length > 0) await onSend(text);
		},
		onCancel,
	});

	return (
		<AssistantRuntimeProvider runtime={runtime}>
			<ThreadPrimitive.Root className="chat-thread">
				<ThreadPrimitive.Viewport className="chat-transcript">
					<ThreadPrimitive.Empty>
						<div className="chat-empty">
							<strong>Ask about this project</strong>
							<span>
								Inspect structure, query notes, explain edits, or create a
								proposal for human review.
							</span>
						</div>
					</ThreadPrimitive.Empty>
					<ThreadPrimitive.Messages components={{ Message: ChatMessage }} />
					{turns.map((turn) =>
						turn.error === undefined ? null : (
							<TurnErrorAlert
								key={`${turn.id}-error`}
								error={turn.error}
								canRetry={canSend}
								onRetry={() => onSend(turn.text)}
							/>
						),
					)}
				</ThreadPrimitive.Viewport>
				<ComposerPrimitive.Root className="chat-composer">
					<ComposerPrimitive.Input
						aria-label="Message the project assistant"
						placeholder="Ask about the project or propose a change"
					/>
					{running ? (
						<ComposerPrimitive.Cancel
							className="chat-composer__action"
							aria-label="Cancel response"
						>
							<Square aria-hidden="true" />
						</ComposerPrimitive.Cancel>
					) : (
						<ComposerPrimitive.Send
							className="chat-composer__action"
							aria-label="Send message"
						>
							<Send aria-hidden="true" />
						</ComposerPrimitive.Send>
					)}
				</ComposerPrimitive.Root>
			</ThreadPrimitive.Root>
		</AssistantRuntimeProvider>
	);
};
