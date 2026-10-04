import { Plus } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "./primitives";

type TimingSectionProps = Readonly<{
	title: string;
	description: string;
	error: string | undefined;
	disabled: boolean;
	onAdd: () => void;
	onSave: () => void;
	children: ReactNode;
}>;

export const TimingSection = ({
	title,
	description,
	error,
	disabled,
	onAdd,
	onSave,
	children,
}: TimingSectionProps) => (
	<section className="timing-section">
		<header>
			<div>
				<h3>{title}</h3>
				<p>{description}</p>
			</div>
			<Button icon={Plus} onClick={onAdd} disabled={disabled}>
				Add point
			</Button>
		</header>
		{children}
		{error === undefined ? null : (
			<p className="timing-error" role="alert">
				{error}
			</p>
		)}
		<Button variant="primary" onClick={onSave} disabled={disabled}>
			{disabled ? "Saving…" : `Save ${title.toLowerCase()}`}
		</Button>
	</section>
);

export type ConstantField = Readonly<{
	id: string;
	label: string;
	value: string;
}>;

type ConstantTimingControlProps = Readonly<{
	label: string;
	fields: readonly ConstantField[];
	onChange: (id: string, value: string) => void;
	onApply: () => void;
	disabled: boolean;
}>;

export const ConstantTimingControl = ({
	label,
	fields,
	onChange,
	onApply,
	disabled,
}: ConstantTimingControlProps) => (
	<div className="constant-timing">
		<span>{label}</span>
		{fields.map((field) => (
			<input
				key={field.id}
				aria-label={field.label}
				type="number"
				value={field.value}
				disabled={disabled}
				onChange={(event) => onChange(field.id, event.currentTarget.value)}
			/>
		))}
		<Button onClick={onApply} disabled={disabled}>Use constant</Button>
	</div>
);
