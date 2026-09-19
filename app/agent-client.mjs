#!/usr/bin/env node
import ky from 'ky';
import { z } from 'zod';

const [projectId, instanceId, ...words] = process.argv.slice(2);
if (projectId === '--help' || !projectId) {
  process.stdout.write('DAIW local assistant\n\nUsage: node app/agent-client.mjs PROJECT_ID INSTANCE_ID "request"\n\nRequires DAIW_AGENT_TOKEN. Optional DAIW_URL (default http://127.0.0.1:3000).\nSupports the deterministic core planner; creates a proposal, never approves it.\nExample requests: "one octave higher", "softer", "brighter".\n');
} else if (!instanceId || words.length === 0 || !process.env.DAIW_AGENT_TOKEN) {
  process.stderr.write('Project, instance, request, and DAIW_AGENT_TOKEN are required. Use --help.\n');
  process.exitCode = 2;
} else {
  const client = ky.create({ prefixUrl: process.env.DAIW_URL ?? 'http://127.0.0.1:3000', timeout: 30000, retry: 0, headers: { authorization: `Bearer ${process.env.DAIW_AGENT_TOKEN}` }, throwHttpErrors: false });
  const envelope = z.discriminatedUnion('ok', [
    z.object({ ok: z.literal(true), state: z.object({ revision: z.string(), proposals: z.array(z.object({ id: z.string(), name: z.string() }).passthrough()) }).passthrough(), result: z.unknown() }),
    z.object({ ok: z.literal(false), error: z.object({ code: z.string(), message: z.string() }) }),
  ]);
  try {
    const query = envelope.parse(await client.post('api/agent', { json: { projectId, method: 'project.explain', params: { instanceId } } }).json());
    if (!query.ok) {
      process.stderr.write(`${query.error.code}: ${query.error.message}\n`);
      process.exitCode = 1;
    } else {
      const proposal = envelope.parse(await client.post('api/agent', { json: { projectId, method: 'proposal.create', params: { expectedRevision: query.state.revision, instanceId, request: words.join(' ') } } }).json());
      if (!proposal.ok) {
        process.stderr.write(`${proposal.error.code}: ${proposal.error.message}\n`);
        process.exitCode = 1;
      } else {
        process.stdout.write(`${JSON.stringify({ proposal: proposal.state.proposals.at(-1), reviewRequired: true }, null, 2)}\n`);
      }
    }
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : 'Agent request failed'}\n`);
    process.exitCode = 1;
  }
}
