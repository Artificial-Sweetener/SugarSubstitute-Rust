/** A fact that never changes. */
export function fixed(value) {
    return { read: () => value, subscribe: () => () => undefined };
}
/** A value and its listeners held in memory, for state a feature computes. */
export class SettingsState {
    commit;
    #listeners = new Set();
    #current;
    constructor(initial, commit = () => undefined) {
        this.commit = commit;
        this.#current = initial;
    }
    read() {
        return this.#current;
    }
    write(value) {
        this.set(value);
        return this.commit(value);
    }
    /** Change the value without committing it, as when its owner reports a new value. */
    set(value) {
        if (Object.is(value, this.#current))
            return;
        this.#current = value;
        for (const listener of [...this.#listeners])
            listener();
    }
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
}
