import { resolveCanvasReveal } from '../domain/CanvasReveal.js';
import { canvasOutputCatalog } from './CanvasOutputCatalog.js';
import { canvasBatchGridCatalog, canvasSelectedItem } from './CanvasContentSelection.js';
/** Keep source preparation fenced to the current workflow, run, and navigation. */
export class CanvasSourceNavigation {
    options;
    #intent = null;
    /** Bind the existing selection, layout, and resource authorities. */
    constructor(options) {
        this.options = options;
    }
    /** Prepare a single final image even from a grid or comparison, fencing every delayed completion. */
    async reveal(target, scope) {
        if (!scope.current())
            return false;
        const initial = this.options.read();
        if (!initial)
            return false;
        const item = resolveCanvasReveal(initial.catalog, target);
        if (!item)
            return false;
        const selection = { sourceId: target.sourceId, batchIndex: target.batchIndex };
        const inspection = canvasOutputCatalog(initial.catalog.result, initial.catalog.sources, selection);
        const intent = {};
        this.#intent = intent;
        const lease = this.#prepare(intent, selection, inspection, null, scope);
        const ready = lease ? await this.#ready(intent, lease) : true;
        if (!this.#current(intent, scope))
            return false;
        const current = this.options.read();
        if (!current ||
            current.plan.signature !== initial.plan.signature ||
            resolveCanvasReveal(current.catalog, target)?.id !== item.id) {
            if (this.#intent === intent)
                this.cancel();
            return false;
        }
        const apply = () => scope.current() &&
            this.options.selection.apply(current.plan.view.viewId, current.catalog, {
                type: 'reveal',
                target,
            });
        if (!this.#current(intent, scope))
            return false;
        this.#intent = null;
        if (!ready)
            return lease?.unavailable ? apply() : false;
        return lease ? lease.commit(apply) : apply();
    }
    /** Prepare a completed source, then commit its normal selection policy synchronously. */
    async select(selection, scope) {
        if (!scope.current())
            return false;
        const initial = this.options.read();
        if (!initial)
            return false;
        const { plan, catalog } = initial;
        const apply = () => scope.current() &&
            this.options.selection.apply(plan.view.viewId, catalog, { type: 'select-source', selection });
        if (plan.view.comparison ||
            catalog.result.runStatus === 'running' ||
            catalog.result.runStatus === 'queued') {
            this.cancel();
            return apply();
        }
        const cells = plan.view.mode === 'overview'
            ? this.options.cells(canvasBatchGridCatalog(catalog, selection, false))
            : null;
        const intent = {};
        this.#intent = intent;
        const lease = this.#prepare(intent, selection, catalog, cells, scope);
        const targets = cells
            ? JSON.stringify(cells.map((cell) => [cell.key, cell.item?.id, cell.preview]))
            : canvasSelectedItem(catalog, selection)?.id;
        if (!this.#current(intent, scope))
            return false;
        if (!lease) {
            this.#intent = null;
            return apply();
        }
        const ready = await this.#ready(intent, lease);
        if (!this.#current(intent, scope))
            return false;
        const current = this.options.read();
        if (!current ||
            current.plan.signature !== plan.signature ||
            current.catalog.generation !== catalog.generation ||
            current.catalog.result.runOrder !== catalog.result.runOrder) {
            if (this.#intent === intent)
                this.cancel();
            return false;
        }
        const currentCells = plan.view.mode === 'overview'
            ? this.options.cells(canvasBatchGridCatalog(current.catalog, selection, false))
            : null;
        const currentTargets = currentCells
            ? JSON.stringify(currentCells.map((cell) => [cell.key, cell.item?.id, cell.preview]))
            : canvasSelectedItem(current.catalog, selection)?.id;
        if (currentTargets !== targets) {
            if (this.#intent === intent)
                this.cancel();
            return false;
        }
        if (!this.#current(intent, scope))
            return false;
        this.#intent = null;
        if (!ready)
            return lease.unavailable ? apply() : false;
        return lease.commit(apply);
    }
    /** Cancel the pending source when another command or surface lifecycle takes precedence. */
    cancel() {
        this.#intent = null;
        this.options.resources.cancelPreparation();
    }
    /** Retire only this still-owned preparation when its originating command loses authority. */
    #current(intent, scope) {
        if (this.#intent !== intent)
            return false;
        if (scope.current() && this.#intent === intent)
            return true;
        if (this.#intent === intent)
            this.cancel();
        return false;
    }
    /** Retire a failed surface admission only if this request still owns it. */
    #prepare(intent, selection, catalog, cells, scope) {
        try {
            return this.options.resources.prepareSource(selection, catalog, cells, scope);
        }
        catch (error) {
            return this.#fail(intent, error);
        }
    }
    /** Preserve a surface failure while retiring only its own still-current stage. */
    async #ready(intent, lease) {
        try {
            return await lease.ready;
        }
        catch (error) {
            return this.#fail(intent, error);
        }
    }
    /** Keep both the preparation and retirement failures observable when either occurs. */
    #fail(intent, error) {
        if (this.#intent === intent) {
            try {
                this.cancel();
            }
            catch (retirement) {
                throw new AggregateError([error, retirement], 'Canvas source preparation retirement failed.');
            }
        }
        throw error;
    }
}
