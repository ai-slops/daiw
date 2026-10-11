import type { ChatScope } from "./chat-state";

type ScopeTicket = {
	readonly scopeGeneration: number;
};

type SessionTicket = ScopeTicket & {
	readonly sessionGeneration: number;
};

const sameScope = (left: ChatScope | null, right: ChatScope | null): boolean =>
	left === null || right === null
		? left === right
		: left.projectId === right.projectId && left.accountId === right.accountId;

export class ChatRequestGeneration {
	#scope: ChatScope | null;
	#scopeGeneration = 0;
	#sessionGeneration = 0;

	constructor(scope: ChatScope | null) {
		this.#scope = scope;
	}

	sync(scope: ChatScope | null): void {
		if (sameScope(this.#scope, scope)) return;
		this.#scope = scope;
		this.#scopeGeneration += 1;
		this.#sessionGeneration += 1;
	}

	scopeTicket(): ScopeTicket {
		return { scopeGeneration: this.#scopeGeneration };
	}

	sessionTicket(): SessionTicket {
		return {
			scopeGeneration: this.#scopeGeneration,
			sessionGeneration: this.#sessionGeneration,
		};
	}

	beginSession(): SessionTicket {
		this.#sessionGeneration += 1;
		return this.sessionTicket();
	}

	isScopeCurrent(ticket: ScopeTicket): boolean {
		return ticket.scopeGeneration === this.#scopeGeneration;
	}

	isSessionCurrent(ticket: SessionTicket): boolean {
		return (
			this.isScopeCurrent(ticket) &&
			ticket.sessionGeneration === this.#sessionGeneration
		);
	}
}
