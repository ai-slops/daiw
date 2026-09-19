import { Bot, Check, GitCompareArrows, RotateCcw, X } from "lucide-react";
import { useMemo, useState } from "react";
import type { AudioEngineState, AuditionSide } from "../audio";
import type { Instance, Proposal } from "../domain/project";
import { ProposalComposer } from "./ProposalComposer";
import { Button, EmptyState, Panel } from "./primitives";

type ProposalPanelProps = {
	readonly instance: Instance | undefined;
	readonly proposals: readonly Proposal[];
	readonly busy: boolean;
	readonly onCreate: (request: string) => void;
	readonly onAccept: (proposalId: string, hunkIds: readonly string[]) => void;
	readonly onReject: (proposalId: string) => void;
	readonly audio: AudioEngineState;
	readonly onPrepareAB: (proposalId: string) => void;
	readonly onSelectAB: (side: AuditionSide) => void;
	readonly onLoudnessMatch: (enabled: boolean) => void;
};

export const ProposalPanel = ({
	instance,
	proposals,
	busy,
	onCreate,
	onAccept,
	onReject,
	audio,
	onPrepareAB,
	onSelectAB,
	onLoudnessMatch,
}: ProposalPanelProps) => {
	const available = proposals.filter(
		(item) => item.instanceId === instance?.id,
	);
	const newest = available.at(-1);
	const [proposalId, setProposalId] = useState<string>();
	const proposal = available.find((item) => item.id === proposalId) ?? newest;
	const [selected, setSelected] = useState<readonly string[]>([]);
	const chosen = useMemo(() => {
		if (proposal === undefined) return new Set<string>();
		const expanded = new Set(selected);
		let previousSize = -1;
		while (previousSize !== expanded.size) {
			previousSize = expanded.size;
			for (const hunk of proposal.hunks) {
				if (!expanded.has(hunk.id)) continue;
				for (const dependency of hunk.dependencies) expanded.add(dependency);
			}
		}
		return expanded;
	}, [proposal, selected]);
	const submitted =
		proposal?.hunks
			.filter((hunk) => chosen.has(hunk.id) && !hunk.accepted)
			.map((hunk) => hunk.id) ?? [];
	const open =
		proposal?.status === "pending" || proposal?.status === "partiallyAccepted";

	if (instance === undefined) {
		return (
			<Panel title="Assistant" eyebrow="Local deterministic">
				<EmptyState>Select a clip to propose scoped changes.</EmptyState>
			</Panel>
		);
	}

	return (
		<Panel
			title="Assistant"
			eyebrow="Local deterministic"
			className="proposal-panel"
		>
			<div className="assistant-note">
				<Bot aria-hidden="true" size="var(--icon-md)" />
				<span>
					This assistant runs the built-in deterministic adapter. It does not
					call an LLM.
				</span>
			</div>
			<ProposalComposer
				instanceName={instance.name}
				busy={busy}
				onCreate={(request) => {
					setProposalId(undefined);
					setSelected([]);
					onCreate(request);
				}}
			/>
			{available.length < 2 ? null : (
				<label className="proposal-input">
					<span>Review proposal</span>
					<select
						value={proposal?.id ?? ""}
						onChange={(event) => {
							setProposalId(event.currentTarget.value);
							setSelected([]);
						}}
					>
						{[...available].reverse().map((item) => (
							<option key={item.id} value={item.id}>
								{item.name} · {item.status}
							</option>
						))}
					</select>
				</label>
			)}

			{proposal === undefined ? (
				<EmptyState>
					No proposal yet. Choose a suggestion or write a supported change.
				</EmptyState>
			) : (
				<article className="proposal-review">
					<header>
						<div>
							<span className="status-pill" data-status={proposal.status}>
								{proposal.status}
							</span>
							<h3>{proposal.name}</h3>
						</div>
						<small>Based on {proposal.baseRevision.slice(0, 8)}</small>
					</header>
					<p>{proposal.request}</p>
					<div className="ab-compare">
						<div>
							<span>A · Current</span>
							<Button
								variant={audio.audition === "a" ? "primary" : "subtle"}
								onClick={() => onSelectAB("a")}
								disabled={audio.audition === null}
							>
								Listen A
							</Button>
						</div>
						<GitCompareArrows aria-hidden="true" size="var(--icon-md)" />
						<div>
							<span>B · Proposal</span>
							<Button
								variant={audio.audition === "b" ? "primary" : "subtle"}
								onClick={() => onSelectAB("b")}
								disabled={audio.audition === null}
							>
								Listen B
							</Button>
						</div>
						<Button
							variant="ghost"
							onClick={() => onPrepareAB(proposal.id)}
							disabled={busy}
						>
							Prepare A/B
						</Button>
						<label className="check-row">
							<input
								type="checkbox"
								checked={audio.loudnessMatching}
								onChange={(event) => onLoudnessMatch(event.target.checked)}
							/>
							Loudness match
						</label>
					</div>
					<fieldset className="hunk-list">
						<legend>Review changes independently</legend>
						{proposal.hunks.map((hunk) => {
							const required =
								chosen.has(hunk.id) && !selected.includes(hunk.id);
							const checked = hunk.accepted || chosen.has(hunk.id);
							return (
								<label className="hunk-row" key={hunk.id}>
									<input
										type="checkbox"
										checked={checked}
										disabled={hunk.accepted || required || !open}
										onChange={(event) =>
											setSelected((current) =>
												event.currentTarget.checked
													? [...current, hunk.id]
													: current.filter((id) => id !== hunk.id),
											)
										}
									/>
									<span>
										<strong>{hunk.title}</strong>
										<small>{hunk.description}</small>
									</span>
									{hunk.accepted ? (
										<em>Applied</em>
									) : required ? (
										<em>Required</em>
									) : null}
								</label>
							);
						})}
					</fieldset>
					<footer>
						<Button
							icon={X}
							variant="danger"
							onClick={() => onReject(proposal.id)}
							disabled={!open}
						>
							Reject
						</Button>
						<Button
							icon={RotateCcw}
							variant="ghost"
							onClick={() => setSelected([])}
						>
							Reset
						</Button>
						<Button
							variant="subtle"
							onClick={() =>
								setSelected(
									proposal.hunks
										.filter((hunk) => !hunk.accepted)
										.map((hunk) => hunk.id),
								)
							}
							disabled={!open}
						>
							Select remaining
						</Button>
						<Button
							icon={Check}
							variant="primary"
							onClick={() => onAccept(proposal.id, submitted)}
							disabled={!open || submitted.length === 0}
						>
							Approve {submitted.length}
						</Button>
					</footer>
				</article>
			)}
		</Panel>
	);
};
