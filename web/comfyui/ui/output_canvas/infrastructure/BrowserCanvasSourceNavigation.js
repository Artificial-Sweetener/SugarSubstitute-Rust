import { canvasBatchGridCatalog, canvasSelectedItem, } from '../application/CanvasContentSelection.js';
/** Keep source preparation fenced to the current workflow, run, and navigation. */
export class BrowserCanvasSourceNavigation {
    options;
    /** Bind the existing selection, layout, and resource authorities. */
    constructor(options) {
        this.options = options;
    }
    /** Prepare a completed source, then commit its normal selection policy synchronously. */
    async select(selection) {
        const initial = this.options.read();
        if (!initial)
            return false;
        const { plan, catalog } = initial;
        const apply = () => this.options.selection.apply(plan.view.viewId, catalog, { type: 'select-source', selection });
        if (plan.view.comparison ||
            catalog.result.runStatus === 'running' ||
            catalog.result.runStatus === 'queued') {
            this.cancel();
            return apply();
        }
        const cells = plan.view.mode === 'overview'
            ? this.options.cells(canvasBatchGridCatalog(catalog, selection, false))
            : null;
        const lease = this.options.resources.prepareSource(selection, catalog, cells);
        const targets = cells
            ? JSON.stringify(cells.map((cell) => [cell.key, cell.item?.id, cell.preview]))
            : canvasSelectedItem(catalog, selection)?.id;
        if (!lease)
            return apply();
        const ready = await lease.ready;
        const current = this.options.read();
        if (!current ||
            current.plan.signature !== plan.signature ||
            current.catalog.generation !== catalog.generation ||
            current.catalog.result.runOrder !== catalog.result.runOrder) {
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
            this.cancel();
            return false;
        }
        if (!ready)
            return lease.unavailable ? apply() : false;
        return lease.commit(apply);
    }
    /** Cancel the pending source when another command or surface lifecycle takes precedence. */
    cancel() {
        this.options.resources.cancelPreparation();
    }
}
