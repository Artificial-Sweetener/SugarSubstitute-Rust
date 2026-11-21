/** Accept stored or requested editor widths in CSS pixels, with zero selecting the default. */
function isEditorWidth(value) {
    return typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 16384;
}
/** Bound persistent history while retaining every live workflow's lightweight choices. */
export class WorkflowViewPreferences {
    write;
    #live = new Map();
    #stored = new Map();
    #selected = null;
    #snapshot = Object.freeze({ canvas: true, masonry: false, editorWidth: 0 });
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
                typeof item.path !== 'string' ||
                !item.path ||
                item.path.length > 4096 ||
                typeof item.canvas !== 'boolean')
                continue;
            this.#stored.set(item.path, Object.freeze({
                canvas: item.canvas,
                // Entries written before editor widths were stored select the default width.
                editorWidth: 'editorWidth' in item && isEditorWidth(item.editorWidth) ? item.editorWidth : 0,
            }));
        }
    }
    /** Read the existing accepted object rather than reconstructing settings during activation. */
    get snapshot() {
        return this.#snapshot;
    }
    /** Capture one explicit live occurrence, including temporary workflows without browser-history keys. */
    snapshotFor(binding, defaults) {
        const current = this.#live.get(binding.id);
        if (current)
            return current.view;
        const view = this.#opening(binding, defaults);
        this.#live.set(binding.id, { path: binding.path, view });
        return view;
    }
    /** Import native checkpoint choices without publishing browser persistence ahead of host durability. */
    restore(binding, snapshot) {
        if (typeof snapshot.canvas !== 'boolean' ||
            typeof snapshot.masonry !== 'boolean' ||
            !isEditorWidth(snapshot.editorWidth))
            throw new TypeError('Restored workflow presentation is invalid.');
        const view = Object.freeze({ ...snapshot });
        this.#live.set(binding.id, { path: binding.path, view });
        if (this.#selected === binding.id)
            this.#snapshot = view;
    }
    /** Select an explicit committed binding and carry choices through rename or Save As. */
    activate(binding, defaults) {
        let current = this.#live.get(binding.id);
        if (!current) {
            const stored = binding.path ? this.#stored.get(binding.path) : undefined;
            current = { path: binding.path, view: this.#opening(binding, defaults) };
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
    /**
     * Record only actual user choices without changing workflow content or graph history.
     *
     * @throws RangeError when an editor width is outside the persisted range.
     */
    change(value) {
        if (value.editorWidth !== undefined && !isEditorWidth(value.editorWidth))
            throw new RangeError('Workflow editor width is invalid.');
        const current = this.#selected ? this.#live.get(this.#selected) : null;
        if (!current)
            return false;
        const next = { ...current.view, ...value };
        if (next.canvas === current.view.canvas &&
            next.masonry === current.view.masonry &&
            next.editorWidth === current.view.editorWidth)
            return false;
        current.view = Object.freeze(next);
        this.#snapshot = current.view;
        this.#persist(current);
        return true;
    }
    /**
     * Release closed in-memory bindings while keeping bounded reopen metadata. A closed workflow's
     * stored choices follow what it last showed, including ones a session restore supplied without
     * publishing them, so the workflow reopens the way it was left.
     */
    retain(ids) {
        const open = new Set(ids);
        for (const [id, closed] of this.#live) {
            if (open.has(id))
                continue;
            this.#live.delete(id);
            const stored = closed.path ? this.#stored.get(closed.path) : undefined;
            if (closed.path &&
                (stored?.canvas !== closed.view.canvas || stored.editorWidth !== closed.view.editorWidth))
                this.#persist(closed);
        }
        if (this.#selected && !open.has(this.#selected))
            this.#selected = null;
    }
    /** The choices a workflow opens with: stored canvas and width, and its own masonry default. */
    #opening(binding, defaults) {
        const stored = binding.path ? this.#stored.get(binding.path) : undefined;
        const fallback = defaults();
        return Object.freeze({
            canvas: stored?.canvas ?? fallback.canvas,
            masonry: fallback.masonry,
            editorWidth: stored?.editorWidth ?? 0,
        });
    }
    /** Persist changed named choices; ordinary tab switching performs no storage writes. */
    #persist(current) {
        if (!current.path)
            return;
        const stored = this.#stored.get(current.path);
        if (stored?.canvas === current.view.canvas && stored.editorWidth === current.view.editorWidth)
            return;
        this.#stored.delete(current.path);
        this.#stored.set(current.path, Object.freeze({ canvas: current.view.canvas, editorWidth: current.view.editorWidth }));
        while (this.#stored.size > 128) {
            const oldest = this.#stored.keys().next().value;
            if (oldest !== undefined)
                this.#stored.delete(oldest);
        }
        this.write([...this.#stored].map(([path, view]) => ({ path, ...view })));
    }
}
