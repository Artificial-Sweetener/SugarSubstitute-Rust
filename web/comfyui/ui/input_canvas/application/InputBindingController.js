import { InputPreviewInterpretation } from './InputPreviewInterpretation.js';
import { inputLabel } from '../domain/InputLoader.js';
import { moveInputEntry } from '../domain/InputBinding.js';
import { chooseCanvasInput, defaultCanvasInput } from '../domain/CanvasInputSource.js';
/** Keep workflow metadata authoritative while source targets stay portable and interpretation-neutral. */
export class InputBindingController {
    loader;
    port;
    initial;
    host;
    nextId;
    report;
    preview;
    #listeners = new Set();
    #stopWidgets;
    #stopCanvas = null;
    #canvas = null;
    #context = null;
    #loading = null;
    #error = null;
    #disposed = false;
    #generation = 0;
    /** Bind persistence and external capabilities without browser or Comfy globals. */
    constructor(loader, port, initial, host, files, nextId, report) {
        this.loader = loader;
        this.port = port;
        this.initial = initial;
        this.host = host;
        this.nextId = nextId;
        this.report = report;
        this.preview = new InputPreviewInterpretation(loader, files, () => port.settings());
        this.#stopWidgets = port.subscribe((intent) => {
            this.adoptFiles(intent);
            this.#notify();
        });
    }
    /** Re-read metadata so native undo owns authored source state. */
    get state() {
        return this.port.read() ?? this.initial;
    }
    /** Expose only initialization as busy; source changes do not edit native document state. */
    get busy() {
        return this.#loading !== null;
    }
    /** Keep unavailable sources explicit instead of showing an unrelated file. */
    get error() {
        return this.#error;
    }
    /** Resolve the attached canvas identity for reusable controls. */
    get canvasId() {
        return this.#canvas?.canvasId ?? '';
    }
    /** Supply layer choices without teaching presentation about native APIs. */
    get context() {
        return this.#context;
    }
    /** Subscribe only to this loader's source and applicable canvas context. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Restore persisted sources or apply the loader-specific source default after canvas authentication. */
    initialize() {
        if (this.#disposed)
            return Promise.resolve();
        if (this.#loading)
            return this.#loading;
        const generation = this.#generation;
        this.#loading = Promise.resolve()
            .then(async () => {
            const canvas = this.host();
            if (canvas !== this.#canvas) {
                this.#stopCanvas?.();
                this.#stopCanvas = null;
                this.#canvas = canvas;
                if (canvas)
                    this.#stopCanvas = canvas.subscribe(() => {
                        void this.#refreshContext().catch(this.report);
                    });
            }
            await this.#refreshContext();
            const saved = await this.port.load();
            if (this.#disposed || generation !== this.#generation)
                return;
            if (saved)
                this.#save(saved);
            else if (canvas && this.#context?.supported) {
                const state = this.state;
                const entries = (state.entries.length
                    ? state.entries
                    : [
                        {
                            id: this.nextId(),
                            label: inputLabel(this.loader.kind) + ' 1',
                            source: { kind: 'file', file: null },
                        },
                    ]).map((entry) => ({
                    ...entry,
                    source: defaultCanvasInput(canvas.canvasId, this.#context, entry.source.kind === 'file' ? entry.source.file : null, this.loader.kind === 'mask' ? 'selection' : 'composition'),
                }));
                this.#save({ ...state, entries, selected: state.selected ?? entries[0]?.id ?? null });
            }
            this.#error = null;
        })
            .catch((error) => {
            if (!this.#disposed) {
                this.#error = error instanceof Error ? error.message : 'Canvas source unavailable.';
                this.report(error);
            }
        })
            .finally(() => {
            this.#loading = null;
            if (!this.#disposed)
                this.#notify();
        });
        this.#notify();
        return this.#loading;
    }
    /** Change a portable source without modifying Photoshop selection or layers. */
    changeSource(source) {
        const state = this.state;
        if (source.kind === 'canvas') {
            if (source.canvasId !== this.canvasId)
                throw new Error('The selected source canvas is unavailable.');
            source = chooseCanvasInput(this.canvasId, this.#context, source.target);
        }
        this.#generation++;
        this.#error = null;
        const selected = state.selected ?? state.entries[0]?.id ?? this.nextId();
        const entries = state.entries.length
            ? state.entries.map((entry) => (entry.id === selected ? { ...entry, source } : entry))
            : [{ id: selected, label: inputLabel(this.loader.kind) + ' 1', source }];
        this.#save({ ...state, entries, selected });
    }
    /** Select an ordered entry without changing native document state. */
    select(id) {
        if (this.state.entries.some((entry) => entry.id === id))
            this.#save({ ...this.state, selected: id });
    }
    /** Add an independent source choice to an ordered loader. */
    add() {
        if (!this.loader.ordered)
            throw new Error('This loader accepts one source.');
        const state = this.state;
        if (state.entries.length >= 64)
            throw new Error('A source collection supports at most 64 entries.');
        const entry = {
            id: this.nextId(),
            label: `${inputLabel(this.loader.kind)} ${String(state.entries.length + 1)}`,
            source: defaultCanvasInput(this.canvasId, this.#context, null, this.loader.kind === 'mask' ? 'selection' : 'composition'),
        };
        this.#save({ ...state, entries: [...state.entries, entry], selected: entry.id });
    }
    /** Remove the selected entry while leaving the connected canvas unchanged. */
    remove() {
        const state = this.state, entries = state.entries.filter((entry) => entry.id !== state.selected);
        this.#save({ ...state, entries, selected: entries[0]?.id ?? null });
    }
    /** Preserve source identity and selection while reordering a batch. */
    move(delta) {
        const state = this.state;
        if (state.selected)
            this.#save(moveInputEntry(state, state.selected, delta));
    }
    /** Adopt explicit graph file edits while preserving SimpleSyrup append intent. */
    adoptFiles(intent = null) {
        const files = this.port.files(), state = this.state;
        if (JSON.stringify(files) === JSON.stringify(state.files))
            return;
        const append = intent === 'append' &&
            state.files.every((file, index) => files[index] === file) &&
            files.length >= state.files.length;
        const existing = [...state.entries];
        const imported = (append ? files.slice(state.files.length) : files).map((file, index) => {
            const found = append
                ? -1
                : existing.findIndex((entry) => entry.source.kind === 'file' && entry.source.file === file);
            return ((found < 0 ? null : existing.splice(found, 1)[0]) ?? {
                id: this.nextId(),
                label: `${inputLabel(this.loader.kind)} ${String((append ? state.entries.length : 0) + index + 1)}`,
                source: { kind: 'file', file },
            });
        });
        const entries = append ? [...state.entries, ...imported] : imported;
        if (entries.length > 64)
            throw new Error('A source collection supports at most 64 entries.');
        this.#generation++;
        this.#save({
            ...state,
            files,
            entries,
            selected: entries.some((entry) => entry.id === state.selected)
                ? state.selected
                : (entries[0]?.id ?? null),
        });
    }
    /** Release observers owned by this exact node lifetime. */
    dispose() {
        this.#disposed = true;
        this.#stopWidgets();
        this.#stopCanvas?.();
        this.#listeners.clear();
    }
    /** Refresh layer choices after committed native changes without pulling pixels. */
    async #refreshContext() {
        const canvas = this.#canvas;
        if (!canvas) {
            this.#context = null;
            return;
        }
        const result = await canvas.execute({ type: 'context' });
        if (this.#disposed || canvas !== this.#canvas)
            return;
        if (result.kind === 'failure')
            throw new Error(result.message);
        this.#context = result.kind === 'context' ? result : null;
        this.#notify();
    }
    /** Persist through native workflow history and notify scoped consumers. */
    #save(value) {
        this.port.write(value);
        this.#notify();
    }
    /** Keep unrelated cards outside this binding's invalidation boundary. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
}
