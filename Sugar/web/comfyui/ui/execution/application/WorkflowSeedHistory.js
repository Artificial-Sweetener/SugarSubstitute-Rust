/** Scope last-used values to an open workflow generation and submission order. */
export class WorkflowSeedHistory {
    #histories = new Map();
    /** Retire closed or replaced workflows while preserving history across undo and tab activation. */
    synchronize(snapshot) {
        const open = new Map(snapshot.open.map((context) => [context.sessionId, context.generation]));
        for (const [id, history] of this.#histories)
            if (open.get(id) !== history.generation)
                this.#histories.delete(id);
        for (const context of snapshot.open)
            if (!this.#histories.has(context.sessionId))
                this.#histories.set(context.sessionId, {
                    generation: context.generation,
                    values: new Map(),
                });
    }
    /** Admit accepted values only to the original open generation; late responses cannot regress history. */
    record(context, order, values) {
        const history = this.#histories.get(context.sessionId);
        if (!history || history.generation !== context.generation)
            return;
        for (const seed of values) {
            const key = JSON.stringify([seed.path, seed.name]);
            const previous = history.values.get(key);
            if (Number.isSafeInteger(seed.value) && (!previous || previous.order < order))
                history.values.set(key, { order, value: seed.value });
        }
    }
    /** Read the newest participating submission, refusing ambiguous shared values. */
    read(context, inputs) {
        const history = this.#histories.get(context.sessionId);
        if (!history || history.generation !== context.generation)
            return null;
        const values = inputs.flatMap((input) => {
            const value = history.values.get(JSON.stringify([input.path, input.name]));
            return value ? [value] : [];
        });
        const order = values.reduce((latest, value) => Math.max(latest, value.order), -Infinity);
        const latest = new Set(values.filter((value) => value.order === order).map((value) => value.value));
        return latest.size === 1 ? ([...latest][0] ?? null) : null;
    }
    /** Release all retained submission values at runtime teardown. */
    dispose() {
        this.#histories.clear();
    }
}
