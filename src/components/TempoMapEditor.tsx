import { Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { Project } from "../domain/project";
import { TempoMapSchema, TempoSchema } from "../domain/project";
import { IconButton } from "./primitives";
import { ConstantTimingControl, TimingSection } from "./TimingEditorPrimitives";

type TempoMap = NonNullable<Project["tempoMap"]>;
type TempoDraft = Readonly<{
	id: string;
	at: string;
	bpm: string;
	curve: "step" | "linear";
}>;

type TempoMapEditorProps = Readonly<{
	tempo: number;
	tempoMap: TempoMap | undefined;
	onMap: (map: TempoMap) => void;
	onConstant: (tempo: number) => void;
	disabled: boolean;
}>;

const tempoDrafts = (
	tempo: number,
	tempoMap: TempoMap | undefined,
): TempoDraft[] =>
	(tempoMap?.points ?? [{ at: 0, bpm: tempo, curve: "step" as const }]).map(
		(point) => ({
			id: crypto.randomUUID(),
			at: String(point.at),
			bpm: String(point.bpm),
			curve: point.curve,
		}),
	);

export const TempoMapEditor = ({
	tempo,
	tempoMap,
	onMap,
	onConstant,
	disabled,
}: TempoMapEditorProps) => {
	const [points, setPoints] = useState(() => tempoDrafts(tempo, tempoMap));
	const [constant, setConstant] = useState(String(tempo));
	const [error, setError] = useState<string>();
	useEffect(() => {
		setPoints(tempoDrafts(tempo, tempoMap));
		setConstant(String(tempo));
		setError(undefined);
	}, [tempo, tempoMap]);
	const parse = () =>
		TempoMapSchema.safeParse({
			points: points.map((point, index) => ({
				at: Number(point.at),
				bpm: Number(point.bpm),
				curve: index === points.length - 1 ? "step" : point.curve,
			})),
		});
	const update = (id: string, patch: Partial<TempoDraft>) =>
		setPoints((current) =>
			current.map((point) =>
				point.id === id ? { ...point, ...patch } : point,
			),
		);

	return (
		<TimingSection
			title="Tempo map"
			description="Beat positions use a 960 tick grid. The final point is always step."
			error={error}
			disabled={disabled}
			onAdd={() => {
				const last = points.at(-1);
				setPoints((current) => [
					...current,
					{
						id: crypto.randomUUID(),
						at: String(Number(last?.at ?? 0) + 4),
						bpm: last?.bpm ?? constant,
						curve: "step",
					},
				]);
			}}
			onSave={() => {
				const result = parse();
				if (!result.success) {
					setError(result.error.issues[0]?.message ?? "Invalid tempo map");
					return;
				}
				setError(undefined);
				onMap(result.data);
			}}
		>
			<div className="timing-table timing-table--tempo">
				<div className="timing-row timing-row--labels">
					<span>Beat</span>
					<span>BPM</span>
					<span>Curve</span>
					<span />
				</div>
				{points.map((point, index) => (
					<div className="timing-row" key={point.id}>
						<input
							aria-label={`Tempo point ${index + 1} beat`}
							type="number"
							min="0"
							step={1 / 960}
							value={point.at}
							disabled={disabled || index === 0}
							onChange={(event) =>
								update(point.id, { at: event.currentTarget.value })
							}
						/>
						<input
							aria-label={`Tempo point ${index + 1} BPM`}
							type="number"
							min="20"
							max="400"
							value={point.bpm}
							disabled={disabled}
							onChange={(event) =>
								update(point.id, { bpm: event.currentTarget.value })
							}
						/>
						<select
							aria-label={`Tempo point ${index + 1} curve`}
							value={index === points.length - 1 ? "step" : point.curve}
							disabled={disabled || index === points.length - 1}
							onChange={(event) =>
								update(point.id, {
									curve:
										event.currentTarget.value === "linear" ? "linear" : "step",
								})
							}
						>
							<option value="step">Step</option>
							<option value="linear">Linear</option>
						</select>
						<IconButton
							label={`Remove tempo point ${index + 1}`}
							icon={Trash2}
							variant="danger"
							disabled={disabled || points.length === 1}
							onClick={() =>
								setPoints((current) =>
									current.filter((item) => item.id !== point.id),
								)
							}
						/>
					</div>
				))}
			</div>
			<ConstantTimingControl
				label="Constant tempo"
				fields={[{ id: "bpm", label: "Constant tempo BPM", value: constant }]}
				onChange={(_id, value) => setConstant(value)}
				disabled={disabled}
				onApply={() => {
					const result = TempoSchema.safeParse(Number(constant));
					if (!result.success) {
						setError(result.error.issues[0]?.message ?? "Invalid tempo");
						return;
					}
					setError(undefined);
					onConstant(result.data);
				}}
			/>
		</TimingSection>
	);
};
