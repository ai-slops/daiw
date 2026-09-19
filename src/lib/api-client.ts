import ky, { HTTPError } from "ky";
import { z } from "zod";
import type { ProjectSummary } from "../domain/api";
import {
	HealthSchema,
	ProjectListSchema,
	SessionSchema,
	WaveformSchema,
} from "../domain/api";
import type { Project } from "../domain/project";
import { RpcResponseSchema } from "../domain/project";

export class ApiError extends Error {
	readonly name = "ApiError";
	constructor(
		readonly code: string,
		message: string,
		options?: ErrorOptions,
	) {
		super(message, options);
	}
}

const ErrorEnvelopeSchema = z.object({
	ok: z.literal(false),
	error: z.object({ code: z.string(), message: z.string() }),
});

const parseHttpError = async (
	error: HTTPError,
	fallbackCode: string,
	fallbackMessage: string,
): Promise<ApiError> => {
	try {
		const envelope = ErrorEnvelopeSchema.parse(
			await error.response.clone().json(),
		);
		return new ApiError(envelope.error.code, envelope.error.message, {
			cause: error,
		});
	} catch {
		return new ApiError(fallbackCode, fallbackMessage, { cause: error });
	}
};

export const apiErrorMessage = (error: unknown): string => {
	if (error instanceof ApiError) return `${error.code}: ${error.message}`;
	if (error instanceof Error) return error.message;
	return "An unknown Studio error occurred.";
};

type RpcOptions = {
	readonly projectId?: string;
	readonly method: string;
	readonly params?: Readonly<Record<string, unknown>>;
	readonly mutation?: boolean;
	readonly idempotencyKey?: string;
};

const http = ky.create({
	timeout: 15_000,
	retry: { limit: 1, methods: ["get"] },
});
let sessionToken: string | undefined;

const getSession = async (): Promise<string> => {
	if (sessionToken !== undefined) return sessionToken;
	const value = SessionSchema.parse(await http.get("api/session").json());
	sessionToken = value.token;
	return value.token;
};

export const listProjects = async (): Promise<readonly ProjectSummary[]> => {
	const value = ProjectListSchema.parse(await http.get("api/projects").json());
	return value.projects;
};

export const getHealth = async (): Promise<number> => {
	const value = HealthSchema.parse(await http.get("api/health").json());
	return value.protocolVersion;
};

export const rpc = async <Output extends z.ZodType>(
	options: RpcOptions,
	resultSchema: Output,
): Promise<{ readonly state: Project; readonly result: z.output<Output> }> => {
	const headers = new Headers();
	headers.set("X-DAIW-Session", await getSession());
	const body = {
		...(options.projectId === undefined
			? {}
			: { projectId: options.projectId }),
		method: options.method,
		params: options.params ?? {},
		...(options.idempotencyKey === undefined
			? {}
			: { idempotencyKey: options.idempotencyKey }),
	};
	try {
		const response = await http.post("api/rpc", { headers, json: body }).json();
		const parsed = RpcResponseSchema.parse(response);
		if (parsed.ok === false)
			throw new ApiError(parsed.error.code, parsed.error.message);
		return { state: parsed.state, result: resultSchema.parse(parsed.result) };
	} catch (error) {
		if (error instanceof ApiError) throw error;
		if (error instanceof HTTPError) {
			throw await parseHttpError(
				error,
				"HttpFailure",
				`Request failed with status ${error.response.status}.`,
			);
		}
		if (error instanceof z.ZodError) {
			throw new ApiError(
				"InvalidResponse",
				"The server returned data this Studio version cannot read.",
				{ cause: error },
			);
		}
		throw error;
	}
};

export const downloadProject = (projectId: string): void => {
	window.location.assign(
		`/api/projects/${encodeURIComponent(projectId)}/export`,
	);
};

export const renderUrl = (projectId: string, proposalId?: string): string => {
	const base = `/api/projects/${encodeURIComponent(projectId)}/render.wav`;
	return proposalId === undefined
		? base
		: `${base}?proposalId=${encodeURIComponent(proposalId)}`;
};

export const downloadMix = (projectId: string): void => {
	const link = document.createElement("a");
	link.href = renderUrl(projectId);
	link.download = "daiw-mix.wav";
	link.click();
};

export const getWaveform = async (
	projectId: string,
	assetId: string,
): Promise<readonly number[]> => {
	const value = WaveformSchema.parse(
		await http
			.get(
				`api/projects/${encodeURIComponent(projectId)}/assets/${encodeURIComponent(assetId)}/peaks`,
			)
			.json(),
	);
	return value.peaks;
};

export const importAudio = async (
	project: Project,
	file: File,
): Promise<Project> => {
	const form = new FormData();
	form.set("file", file);
	const track = project.tracks[0];
	if (track === undefined)
		throw new ApiError("MissingTrack", "Add a track before importing audio.");
	form.set("trackId", track.id);
	form.set("start", "0");
	form.set("expectedRevision", project.revision);
	const token = await getSession();
	const idempotencyKey = crypto.randomUUID();
	try {
		const response = await http
			.post(`api/projects/${encodeURIComponent(project.id)}/import-audio`, {
				body: form,
				headers: {
					"X-DAIW-Session": token,
					"X-DAIW-Idempotency": idempotencyKey,
				},
			})
			.json();
		const parsed = RpcResponseSchema.parse(response);
		if (parsed.ok === false)
			throw new ApiError(parsed.error.code, parsed.error.message);
		return parsed.state;
	} catch (error) {
		if (error instanceof ApiError) throw error;
		if (error instanceof HTTPError) {
			throw await parseHttpError(
				error,
				"UploadFailure",
				`Audio import failed with status ${error.response.status}.`,
			);
		}
		if (error instanceof z.ZodError) {
			throw new ApiError(
				"InvalidResponse",
				"The imported project response could not be read.",
				{ cause: error },
			);
		}
		throw error;
	}
};
