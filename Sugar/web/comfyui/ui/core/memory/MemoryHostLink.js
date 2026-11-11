import { preferenceMessage, readAllowance, usageMessage, } from './MemoryHostProtocol.js';
/**
 * Report cache usage and the stored preference to the host, and apply the host's allowance.
 *
 * A host that does not speak the protocol rejects the first report; the link then stops and
 * the registry keeps its standalone allowance.
 */
export class MemoryHostLink {
    transport;
    registry;
    preferences;
    report;
    #stops = [];
    #generation = 0;
    #enabled = true;
    constructor(transport, registry, preferences, report) {
        this.transport = transport;
        this.registry = registry;
        this.preferences = preferences;
        this.report = report;
    }
    /** Listen for allowances before announcing the preference and current usage. */
    start() {
        if (this.#stops.length > 0)
            return;
        const session = this.transport.session;
        this.#stops.push(this.transport.subscribe((value) => {
            const allowance = readAllowance(value, session);
            if (!allowance || allowance.generation <= this.#generation)
                return;
            this.#generation = allowance.generation;
            this.registry.setAllowance(allowance.allowanceBytes, allowance.level);
        }), this.registry.subscribe((usage) => this.#send(usageMessage(session, usage))), this.preferences.subscribe((preference) => this.#send(preferenceMessage(session, preference))));
        this.#send(preferenceMessage(session, this.preferences.read()));
        this.#send(usageMessage(session, this.registry.usage()));
    }
    /** Stop reporting; the registry keeps the last allowance it received. */
    dispose() {
        for (const stop of this.#stops.splice(0))
            stop();
    }
    #send(message) {
        if (!this.#enabled)
            return;
        try {
            this.transport.send(message);
        }
        catch (error) {
            this.#enabled = false;
            this.dispose();
            this.report(error);
        }
    }
}
