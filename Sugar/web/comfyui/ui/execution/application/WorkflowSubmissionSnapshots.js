/** Keep checkpoint receipts after transport routing retires without retaining host graphs or pixels. */
export class WorkflowSubmissionSnapshots {
    #entries = new Map();
    #listeners = new Set();
    /** Commit before final output; prompt association is itself a durability-significant transition. */
    accept(submission) {
        let prompts = this.#entries.get(submission.entryId);
        const previous = prompts?.get(submission.promptId);
        if (previous) {
            if (JSON.stringify(previous) !== JSON.stringify(submission))
                throw new TypeError('An accepted prompt cannot change its captured ownership.');
            return;
        }
        for (const entry of this.#entries.values())
            if (entry.has(submission.promptId))
                throw new TypeError('An accepted prompt cannot belong to multiple desktop occurrences.');
        prompts ??= new Map();
        prompts.set(submission.promptId, Object.freeze({
            ...submission,
            scene: submission.scene ? Object.freeze({ ...submission.scene }) : null,
            sources: Object.freeze(submission.sources.map((source) => Object.freeze({ ...source }))),
        }));
        this.#entries.set(submission.entryId, prompts);
        this.#publish();
    }
    /** Read accepted pending and completed associations without depending on backend history. */
    read(entryId) {
        return Object.freeze([...(this.#entries.get(entryId)?.values() ?? [])].sort((a, b) => a.runOrder - b.runOrder));
    }
    /** Resolve the frozen source captured for one accepted prompt, never the current reused node. */
    source(entryId, prompt, executionPath) {
        return (this.#entries
            .get(entryId)
            ?.get(prompt)
            ?.sources.find((source) => source.executionPath === executionPath) ?? null);
    }
    /** Import association metadata without publishing a generation or creating live transport jobs. */
    restore(entryId, submissions) {
        if (submissions.some((submission) => submission.entryId !== entryId))
            throw new TypeError('Restored prompt association names another desktop occurrence.');
        const next = new Map();
        for (const submission of submissions) {
            if (next.has(submission.promptId) ||
                [...this.#entries].some(([id, prompts]) => id !== entryId && prompts.has(submission.promptId)))
                throw new TypeError('Restored prompt ownership is ambiguous.');
            next.set(submission.promptId, Object.freeze({
                ...submission,
                scene: submission.scene ? Object.freeze({ ...submission.scene }) : null,
                sources: Object.freeze(submission.sources.map((source) => Object.freeze({ ...source }))),
            }));
        }
        this.#entries.set(entryId, next);
        this.#publish();
    }
    /** Remove closed occurrences while ordinary completed-job retirement leaves receipts intact. */
    synchronize(bindings) {
        const open = new Set(bindings.map((binding) => binding.entryId));
        let changed = false;
        for (const entry of this.#entries.keys())
            if (!open.has(entry)) {
                this.#entries.delete(entry);
                changed = true;
            }
        if (changed)
            this.#publish();
    }
    /** Retain exactly the pending/current scene and visible/comparison prompt memberships chosen by results. */
    retain(entryId, promptIds) {
        const prompts = this.#entries.get(entryId);
        if (!prompts)
            return;
        let changed = false;
        for (const prompt of prompts.keys())
            if (!promptIds.has(prompt)) {
                prompts.delete(prompt);
                changed = true;
            }
        if (changed)
            this.#publish();
    }
    /** Notify the checkpoint owner before any final media is required. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Release all metadata at page teardown without changing any backend job. */
    dispose() {
        this.#entries.clear();
        this.#listeners.clear();
    }
    /** Keep observation distinct from execution delivery and editor insertion. */
    #publish() {
        for (const listener of this.#listeners)
            listener();
    }
}
