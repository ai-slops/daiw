import { Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { Project } from "../domain/project";
import { MeterMapSchema, MeterSchema } from "../domain/project";
import { IconButton } from "./primitives";
import { ConstantTimingControl, TimingSection } from "./TimingEditorPrimitives";
import { nextBar } from "./transport-timing";

type MeterMap = NonNullable<Project["meterMap"]>;
type MeterDraft = Readonly<{
	id: string;
	at: string;
	numerator: string;
	denominator: string;
}>;

type MeterMapEditorProps = Readonly<{
	meter: Project["meter"];
	meterMap: MeterMap | undefined;
	onMap: (map: MeterMap) => void;
	onConstant: (meter: Project["meter"]) => void;
	disabled: boolean;
}>;

const meterDrafts = (
	meter: Project["meter"],
	meterMap: MeterMap | undefined,
): MeterDraft[] =>
	(
		meterMap?.points ?? [{ at: 0, numerator: meter[0], denominator: meter[1] }]
	).map((point) => ({
		id: crypto.randomUUID(),
		at: String(point.at),
		numerator: String(point.numerator),
		denominator: String(point.denominator),
	}));

export const MeterMapEditor = ({
	meter,
	meterMap,
	onMap,
	onConstant,
	disabled,
}: MeterMapEditorProps) => {
	const [points, setPoints] = useState(() => meterDrafts(meter, meterMap));
	const [numerator, setNumerator] = useState(String(meter[0]));
	const [denominator, setDenominator] = useState(String(meter[1]));
	const [error, setError] = useState<string>();
	useEffect(() => {
		setPoints(meterDrafts(meter, meterMap));
		setNumerator(String(meter[0]));
		setDenominator(String(meter[1]));
		setError(undefined);
	}, [meter, meterMap]);
	const parse = () =>
		MeterMapSchema.safeParse({
			points: points.map((point) => ({
				at: Number(point.at),
				numerator: Number(point.numerator),
				denominator: Number(point.denominator),
			})),
		});
	const update = (id: string, patch: Partial<MeterDraft>) =>
		setPoints((current) =>
			current.map((point) =>
				point.id === id ? { ...point, ...patch } : point,
			),
		);

	return (
		<TimingSection
			title="Meter map"
			description="Changes must land on a previous-meter bar boundary."
			error={error}
			disabled={disabled}
			onAdd={() => {
				const result = parse();
				if (!result.success) {
					setError(
						result.error.issues[0]?.message ??
							"Fix the meter map before adding a point",
					);
					return;
				}
				const last = points.at(-1);
				setError(undefined);
				setPoints((current) => [
					...current,
					{
						id: crypto.randomUUID(),
						at: String(nextBar(Number(last?.at ?? 0), result.data)),
						numerator: last?.numerator ?? numerator,
						denominator: last?.denominator ?? denominator,
					},
				]);
			}}
			onSave={() => {
				const result = parse();
				if (!result.success) {
					setError(result.error.issues[0]?.message ?? "Invalid meter map");
					return;
				}
				setError(undefined);
				onMap(result.data);
			}}
		>
			<div className="timing-table timing-table--meter">
				<div className="timing-row timing-row--labels">
					<span>Beat</span>
					<span>Numerator</span>
					<span>Denominator</span>
					<span />
				</div>
				{points.map((point, index) => (
					<div className="timing-row" key={point.id}>
						<input
							aria-label={`Meter point ${index + 1} beat`}
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
							aria-label={`Meter point ${index + 1} numerator`}
							type="number"
							min="1"
							max="255"
							value={point.numerator}
							disabled={disabled}
							onChange={(event) =>
								update(point.id, { numerator: event.currentTarget.value })
							}
						/>
						<input
							aria-label={`Meter point ${index + 1} denominator`}
							type="number"
							min="1"
							max="255"
							value={point.denominator}
							disabled={disabled}
							onChange={(event) =>
								update(point.id, { denominator: event.currentTarget.value })
							}
						/>
						<IconButton
							label={`Remove meter point ${index + 1}`}
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
				label="Constant meter"
				fields={[
					{
						id: "numerator",
						label: "Constant meter numerator",
						value: numerator,
					},
					{
						id: "denominator",
						label: "Constant meter denominator",
						value: denominator,
					},
				]}
				onChange={(id, value) =>
					id === "numerator" ? setNumerator(value) : setDenominator(value)
				}
				disabled={disabled}
				onApply={() => {
					const result = MeterSchema.safeParse([
						Number(numerator),
						Number(denominator),
					]);
					if (!result.success) {
						setError(result.error.issues[0]?.message ?? "Invalid meter");
						return;
					}
					setError(undefined);
					onConstant(result.data);
				}}
			/>
		</TimingSection>
	);
};
