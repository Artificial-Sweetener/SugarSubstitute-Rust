/** Bound persistent history while retaining every live workflow's lightweight choices. */
export class WorkflowViewPreferences {
    write;
    #live = new Map();
    #stored = new Map();
    #selected = null;
    #snapshot = Object.freeze({ canvas: true, masonry: false });
    /** Validate the persistence boundary once and keep tab activation entirely in memory. */
    constructor(value, write) {
        this.write = write;
        if (!Array.isArray(value))
            return;
        for (const item of value.slice(-128)) {
            if (typeof item !== 'object' ||
                item === null ||
                !('path' in item) ||
                !('canvas' in item) ||
                !('masonry' in item) ||
                typeof item.path !== 'string' ||
                !item.path ||
                item.path.length > 4096 ||
                typeof item.canvas !== 'boolean' ||
                typeof item.masonry !== 'boolean')
                continue;
            this.#stored.set(item.path, Object.freeze({ canvas: item.canvas, masonry: item.masonry }));
        }
    }
    /** Read the existing accepted object rather than reconstructing settings during activation. */
    get snapshot() {
        return this.#snapshot;
    }
    /** Select an explicit committed binding and carry choices through rename or Save As. */
    activate(binding, defaultCanvas) {
        let current = this.#live.get(binding.id);
        if (!current) {
            const stored = binding.path ? this.#stored.get(binding.path) : undefined;
            current = {
                path: binding.path,
                view: stored || Object.freeze({ canvas: defaultCanvas(), masonry: false }),
            };
            this.#live.set(binding.id, current);
            if (!stored)
                this.#persist(current);
        }
        else if (current.path !== binding.path) {
            current.path = binding.path;
            this.#persist(current);
        }
        const changed = this.#selected !== binding.id || this.#snapshot !== current.view;
        this.#selected = binding.id;
        this.#snapshot = current.view;
        return changed;
    }
    /** Record only actual user choices without changing workflow content or graph history. */
    change(value) {
        const current = this.#selected ? this.#live.get(this.#selected) : null;
        if (!current)
            return false;
        const next = { ...current.view, ...value };
        if (next.canvas === current.view.canvas && next.masonry === current.view.masonry)
            return false;
        current.view = Object.freeze(next);
        this.#snapshot = current.view;
        this.#persist(current);
        return true;
    }
    /** Release closed in-memory bindings while keeping bounded reopen metadata. */
    retain(ids) {
        const open = new Set(ids);
        for (const id of this.#live.keys())
            if (!open.has(id))
                this.#live.delete(id);
        if (this.#selected && !open.has(this.#selected))
            this.#selected = null;
    }
    /** Persist changed named choices; ordinary tab switching performs no storage writes. */
    #persist(current) {
        if (!current.path)
            return;
        this.#stored.delete(current.path);
        this.#stored.set(current.path, current.view);
        while (this.#stored.size > 128) {
            const oldest = this.#stored.keys().next().value;
            if (oldest !== undefined)
                this.#stored.delete(oldest);
        }
        this.write([...this.#stored].map(([path, view]) => ({ path, ...view })));
    }
}
