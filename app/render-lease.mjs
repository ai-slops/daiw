export const VERIFIED_RENDER_LEASE_POLICY = Object.freeze({
	maximumEntries: 16,
	ttlMilliseconds: 60_000,
});

function leaseKey({ projectId, proposalId, revision, renderKey }) {
	return JSON.stringify([projectId, proposalId, revision, renderKey]);
}

export class VerifiedRenderLeases {
	#entries = new Map();

	get(binding) {
		const key = leaseKey(binding);
		const entry = this.#entries.get(key);
		if (!entry) return undefined;
		if (entry.expiresAt <= Date.now()) {
			this.#entries.delete(key);
			return undefined;
		}
		this.#entries.delete(key);
		this.#entries.set(key, entry);
		return entry;
	}

	remember(binding, rendered) {
		const key = leaseKey(binding);
		this.#entries.delete(key);
		while (this.#entries.size >= VERIFIED_RENDER_LEASE_POLICY.maximumEntries) {
			this.#entries.delete(this.#entries.keys().next().value);
		}
		this.#entries.set(key, {
			revision: rendered.revision,
			key: rendered.key,
			descriptor: rendered.descriptor,
			analysis: rendered.analysis,
			fileIdentity: rendered.fileIdentity,
			expiresAt: Date.now() + VERIFIED_RENDER_LEASE_POLICY.ttlMilliseconds,
		});
	}

	invalidate(binding) {
		this.#entries.delete(leaseKey(binding));
	}
}
