import { Upload } from "lucide-react";
import { useRef, useState } from "react";
import { Button } from "./primitives";

type ProjectImportControlsProps = {
	readonly busy: boolean;
	readonly onClearError: () => void;
	readonly onImport: (file: File) => Promise<boolean>;
	readonly onImported: () => void;
};

export const ProjectImportControls = ({
	busy,
	onClearError,
	onImport,
	onImported,
}: ProjectImportControlsProps) => {
	const inputRef = useRef<HTMLInputElement>(null);
	const [importing, setImporting] = useState(false);

	return (
		<>
			<input
				ref={inputRef}
				className="sr-only"
				type="file"
				accept="application/json,.json,.daiw"
				aria-label="Import DAIW project file"
				onChange={(event) => {
					const file = event.currentTarget.files?.[0];
					event.currentTarget.value = "";
					if (file === undefined) return;
					setImporting(true);
					void onImport(file).then((imported) => {
						setImporting(false);
						if (imported) onImported();
					});
				}}
			/>
			<Button
				icon={Upload}
				disabled={busy || importing}
				onClick={() => {
					onClearError();
					inputRef.current?.click();
				}}
			>
				{importing ? "Importing project…" : "Import project"}
			</Button>
		</>
	);
};
