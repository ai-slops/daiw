import { Sparkles } from "lucide-react";
import { useState } from "react";
import { Button } from "./primitives";

const suggestions = [
	"An octave up",
	"Softer and more dynamic",
	"Brighter",
	"Humanize velocity",
] as const;

type ProposalComposerProps = {
	readonly instanceName: string;
	readonly busy: boolean;
	readonly onCreate: (request: string) => void;
};

export const ProposalComposer = ({
	instanceName,
	busy,
	onCreate,
}: ProposalComposerProps) => {
	const [request, setRequest] = useState("");
	return (
		<>
			<fieldset className="suggestion-row">
				<legend className="sr-only">Prompt suggestions</legend>
				{suggestions.map((suggestion) => (
					<button
						type="button"
						key={suggestion}
						onClick={() => setRequest(suggestion)}
					>
						{suggestion}
					</button>
				))}
			</fieldset>
			<label className="proposal-input">
				<span>Change {instanceName}</span>
				<textarea
					value={request}
					onChange={(event) => setRequest(event.target.value)}
					placeholder="Try: make this brighter"
					rows={2}
				/>
			</label>
			<Button
				icon={Sparkles}
				variant="primary"
				disabled={busy || request.trim().length === 0}
				onClick={() => onCreate(request.trim())}
			>
				Create proposal
			</Button>
		</>
	);
};
