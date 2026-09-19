import ky from "ky";

type ServerRenderRequest = Readonly<{
  projectId: string;
  expectedRevision: string;
  proposalId?: string;
}>;

export class ServerRenderError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "ServerRenderError";
  }
}

export function renderRevisionMatches(expected: string, received: string | null): boolean {
  return received === expected;
}

export function snapshotRevisionMatches(captured: string, current: string | null): boolean {
  return current === captured;
}

export async function fetchServerRender(
  context: AudioContext,
  request: ServerRenderRequest,
): Promise<AudioBuffer> {
  const url = `/api/projects/${encodeURIComponent(request.projectId)}/render.wav`;
  const searchParams = new URLSearchParams({ expectedRevision: request.expectedRevision });
  if (request.proposalId !== undefined) searchParams.set("proposalId", request.proposalId);
  const response = await ky.get(url, { searchParams });
  const received = response.headers.get("X-DAIW-Revision");
  if (!renderRevisionMatches(request.expectedRevision, received)) {
    throw new ServerRenderError(
      `render revision mismatch: expected ${request.expectedRevision}, received ${received ?? "missing"}`,
    );
  }
  return context.decodeAudioData(await response.arrayBuffer());
}
