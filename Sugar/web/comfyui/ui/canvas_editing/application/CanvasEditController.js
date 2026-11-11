import { DEFAULT_COVERAGE_MODIFICATION, } from '../domain/CanvasCoverageModification.js';
import { canvasEditKeyIntent } from '../domain/CanvasEditKeys.js';
import { canvasEditSurface } from '../domain/CanvasEditSurface.js';
import { canvasEditSlotOf, canvasEditSlotViews, canvasEditToolAvailable, } from '../domain/CanvasEditTools.js';
import { CanvasEditCommandQueue } from './CanvasEditCommandQueue.js';
/**
 * Own transient editing-chrome state while the host owns the document.
 *
 * The controller renders only from host states, newest revision first, and keeps just what the
 * chrome itself decides: the remembered member of each tool group, the modification amounts being
 * previewed, and whether a selection modification page is open. Nothing is shown until the host
 * presents an editable document.
 */
export class CanvasEditController {
    options;
    host;
    #queue;
    #listeners = new Set();
    #remembered = new Map();
    #subscription = null;
    #state = null;
    #snapshot = null;
    #modifying = false;
    #modify = DEFAULT_COVERAGE_MODIFICATION;
    #coverage = DEFAULT_COVERAGE_MODIFICATION;
    #navigationHeld = false;
    #disposed = false;
    constructor(options) {
        this.options = options;
        this.host = options.host;
        this.#queue = new CanvasEditCommandQueue(options.host, {
            adopt: (state) => this.#adopt(state),
            report: (error, command) => options.report(error, command),
        });
    }
    /** Observe pushed states and request the current one, since the host pushes only changes. */
    start() {
        if (this.#disposed || this.#subscription)
            return;
        this.#subscription = this.host.changes.subscribe((state) => this.#adopt(state));
        void this.#subscription.ready.then((status) => {
            if (status !== 'observing' && !this.#disposed)
                this.options.report(new Error(`Canvas editing observation is ${status}.`), null);
        });
        void this.#queue.send({ type: 'state' });
    }
    /** Read the latest snapshot; null until the host presents an editable document. */
    read() {
        return this.#snapshot;
    }
    /** Notify a listener after every snapshot change; returns its release. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Activate the member a rail slot presents. */
    selectSlot(slot) {
        const view = this.#snapshot?.slots.find((candidate) => candidate.id === slot);
        if (view?.current.available)
            this.selectTool(view.current.id);
    }
    /** Activate a tool and remember it as its group's presented member. */
    selectTool(tool) {
        const state = this.#state;
        if (!state || !canvasEditToolAvailable(state, tool))
            return;
        const slot = canvasEditSlotOf(tool);
        if (slot)
            this.#remembered.set(slot, tool);
        this.#publish();
        void this.#queue.send({ type: 'tool', tool });
    }
    /** Replace brush or eraser options; drags send only their latest value. */
    setBrushOptions(tool, options) {
        void this.#queue.send({ type: 'options', tool, options }, `options:${tool}`);
    }
    /** Send a one-shot command from a toolbar button. */
    execute(command) {
        void this.#queue.send(command);
    }
    /** Open the selection modification page and preview its initial amount at once. */
    beginModify() {
        if (!this.#state?.selection || this.#modifying)
            return;
        this.#modifying = true;
        this.#modify = DEFAULT_COVERAGE_MODIFICATION;
        this.#publish();
        this.#previewModify();
    }
    /** Replace the selection preview from its original with a new operation or amount. */
    updateModify(value) {
        if (!this.#modifying)
            return;
        this.#modify = value;
        this.#publish();
        this.#previewModify();
    }
    /** Commit the latest selection preview once. */
    async applyModify() {
        if (!this.#modifying)
            return;
        const result = await this.#queue.send(this.#modifyCommand('apply'));
        if (result?.kind === 'state')
            this.#closeModify();
    }
    /** Restore the original selection and close the page. */
    cancelModify() {
        if (!this.#modifying && this.#state?.session?.kind !== 'modify')
            return;
        void this.#queue.send(this.#modifyCommand('cancel'));
        this.#closeModify();
    }
    /** Enter the exclusive whole-mask coverage editor and preview its initial amount. */
    async openCoverage() {
        if (!this.#state?.activeMask || this.#state.mode === 'coverage')
            return;
        this.#coverage = DEFAULT_COVERAGE_MODIFICATION;
        const result = await this.#queue.send({ type: 'mode', mode: 'coverage' });
        if (result?.kind === 'state' && result.state.mode === 'coverage')
            this.#previewCoverage();
    }
    /** Replace the whole-mask preview with a new operation or amount. */
    updateCoverage(value) {
        if (this.#state?.mode !== 'coverage')
            return;
        this.#coverage = value;
        this.#publish();
        this.#previewCoverage();
    }
    /** Commit the whole-mask preview, then leave the exclusive editor. */
    async applyCoverage() {
        if (this.#state?.mode !== 'coverage')
            return;
        const result = await this.#queue.send(this.#coverageCommand('apply'));
        if (result?.kind === 'state')
            void this.#queue.send({ type: 'mode', mode: 'none' });
    }
    /** Discard the whole-mask preview and leave the exclusive editor. */
    closeCoverage() {
        if (this.#state?.mode !== 'coverage')
            return;
        void this.#queue.send(this.#coverageCommand('cancel'));
        void this.#queue.send({ type: 'mode', mode: 'none' });
    }
    /** Apply one keyboard shortcut; returns whether the canvas consumed the key. */
    key(key) {
        const snapshot = this.#snapshot;
        if (!snapshot)
            return false;
        const intent = canvasEditKeyIntent(key, snapshot.state, snapshot.surface);
        if (!intent)
            return false;
        if (intent.kind === 'close-coverage')
            this.closeCoverage();
        else if (intent.kind === 'cancel-modify')
            this.cancelModify();
        else if (intent.kind === 'apply-modify')
            void this.applyModify();
        else if (intent.command.type === 'navigation')
            this.#navigate(intent.command.held);
        else
            this.execute(intent.command);
        return true;
    }
    /** Release temporary navigation when the page loses keyboard focus mid-hold. */
    releaseNavigation() {
        if (this.#navigationHeld)
            this.#navigate(false);
    }
    /** Stop observation and drop unsent commands. */
    dispose() {
        if (this.#disposed)
            return;
        this.releaseNavigation();
        this.#disposed = true;
        this.#subscription?.stop();
        this.#subscription = null;
        this.#queue.dispose();
        this.#listeners.clear();
    }
    #navigate(held) {
        if (held === this.#navigationHeld)
            return;
        this.#navigationHeld = held;
        this.execute({ type: 'navigation', held });
    }
    #previewModify() {
        void this.#queue.send(this.#modifyCommand('preview'), 'modify:selection');
    }
    #previewCoverage() {
        void this.#queue.send(this.#coverageCommand('preview'), 'modify:mask');
    }
    #modifyCommand(phase) {
        return { type: 'modify', target: 'selection', ...this.#modify, phase };
    }
    #coverageCommand(phase) {
        return { type: 'modify', target: 'mask', ...this.#coverage, phase };
    }
    #closeModify() {
        if (!this.#modifying)
            return;
        this.#modifying = false;
        this.#publish();
    }
    /** Adopt only newer revisions; a resolved modification session closes its page. */
    #adopt(state) {
        if (this.#disposed || (this.#state && state.revision <= this.#state.revision))
            return;
        const previous = this.#state;
        this.#state = state;
        if (this.#modifying && previous?.session?.kind === 'modify' && state.session?.kind !== 'modify')
            this.#modifying = false;
        if (this.#modifying && !state.selection && state.session?.kind !== 'modify')
            this.#modifying = false;
        this.#publish();
    }
    #publish() {
        const state = this.#state;
        this.#snapshot =
            state && state.document !== null
                ? Object.freeze({
                    state,
                    surface: canvasEditSurface(state, this.#modifying),
                    slots: canvasEditSlotViews(state, this.#remembered),
                    modify: this.#modify,
                    coverage: this.#coverage,
                })
                : null;
        for (const listener of [...this.#listeners])
            listener();
    }
}
