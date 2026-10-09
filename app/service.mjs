import { createHash, randomUUID } from 'node:crypto';
import { z } from 'zod';
import { CoreWorker } from './worker.mjs';
import { ProjectStore } from './storage.mjs';
import { ServiceError } from './errors.mjs';

export const rpcSchema = z.object({
  projectId: z.string().min(1).max(160).optional(),
  method: z.enum(['project.create', 'project.import', 'project.export', 'project.get', 'project.queryNotes', 'project.validate', 'project.snapshot', 'project.explain', 'project.hash', 'project.history', 'project.cacheStats', 'change.dryRun', 'change.apply', 'proposal.create', 'proposal.accept', 'proposal.reject', 'project.undo', 'project.redo']),
  params: z.record(z.string(), z.unknown()).default({}),
  idempotencyKey: z.string().min(1).max(160).optional(),
});
const queries = new Set(['project.get', 'project.export', 'project.queryNotes', 'project.validate', 'project.snapshot', 'project.explain', 'project.hash', 'project.history', 'project.cacheStats', 'change.dryRun']);
const agentMethods = new Set(['project.get', 'project.export', 'project.queryNotes', 'project.validate', 'project.snapshot', 'project.explain', 'project.hash', 'project.history', 'change.dryRun', 'proposal.create']);

export class ProjectService {
  #queue = Promise.resolve();
  constructor({ directory, binary }) {
    this.store = new ProjectStore(directory);
    this.worker = new CoreWorker(binary);
  }
  execute(request, principal = 'human') {
    const next = this.#queue.then(() => this.#execute(request, principal));
    this.#queue = next.then(() => undefined, () => undefined);
    return next;
  }
  async #execute(request, principal) {
    const { projectId, method, params } = rpcSchema.parse(request);
    if (principal === 'agent' && !agentMethods.has(method)) throw new ServiceError('PermissionDenied', 'Agents can query and propose but cannot change main or approve proposals.', 403);
    const isCreation = method === 'project.create' || method === 'project.import';
    if (!isCreation && !projectId) throw new ServiceError('MissingProject', 'Select a project first.');
    if ('snapshot' in params) throw new ServiceError('PermissionDenied', 'Raw state replacement is not a public command.', 403);
    const requestId = request.idempotencyKey ?? randomUUID();
    const requestHash = createHash('sha256').update(JSON.stringify({ projectId, method, params, principal })).digest('hex');
    if (!queries.has(method)) {
      const prior = this.store.replay(projectId ?? '', requestId, requestHash);
      if (prior) return prior;
    }
    const state = isCreation ? null : this.store.get(projectId);
    let inputParams = params;
    switch (method) {
      case 'project.undo': inputParams = { ...params, snapshot: this.store.historyTarget(projectId, 'undo') }; break;
      case 'project.redo': inputParams = { ...params, snapshot: this.store.historyTarget(projectId, 'redo') }; break;
      default: break;
    }
    const response = await this.worker.request({ principal, state, method, params: inputParams });
    if (response.ok && !queries.has(method)) {
      this.store.commit({ response, base: state, requestId, requestHash, method, principal });
    }
    return response;
  }
  async close() {
    await this.#queue;
    this.worker.close();
    this.store.close();
  }
}
