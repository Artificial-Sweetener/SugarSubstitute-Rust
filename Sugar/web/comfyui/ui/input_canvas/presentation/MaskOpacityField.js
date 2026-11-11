import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';
/** Quiet period after the last slider value before the opacity becomes one canvas step. */
const COMMIT_DELAY_MS = 200;
/**
 * Show the field only while the loader has masks on a workflow-surface canvas.
 *
 * Slider drags emit many values; only the value at rest is applied, so one drag becomes one
 * undoable canvas step.
 */
export class MaskOpacityField {
    surface;
    fields;
    label;
    report;
    #stop;
    #mount = null;
    #pending = null;
    #timer = null;
    #alive = true;
    /** Follow provisioning changes for this loader's card. */
    constructor(surface, fields, label, report) {
        this.surface = surface;
        this.fields = fields;
        this.label = label;
        this.report = report;
        this.#stop = surface.subscribe(() => this.refresh());
        this.refresh();
    }
    /** Mount or release the field and show the stored percentage. */
    refresh() {
        if (!this.#alive)
            return;
        const opacity = this.surface.opacity();
        if (opacity === null) {
            this.#mount?.dispose();
            this.#mount = null;
            return;
        }
        this.#mount ??= this.fields.mountNumber(this.label, (value) => this.#change(value), { min: 0, max: 100, step: 1, precision: 0, presentation: 'spinner-slider' }, 'mask-visual-opacity');
        this.#mount.update(this.#pending ?? Math.round(opacity * 100));
    }
    /** Apply a pending value immediately and release the field. */
    dispose() {
        this.#flush();
        this.#alive = false;
        this.#stop();
        this.#mount?.dispose();
        this.#mount = null;
    }
    /** Hold the newest value until the slider rests. */
    #change(value) {
        this.#pending = Math.min(100, Math.max(0, Math.round(value)));
        if (this.#timer !== null)
            clearTimeout(this.#timer);
        this.#timer = setTimeout(() => this.#flush(), COMMIT_DELAY_MS);
    }
    /** Apply the held value as one change. */
    #flush() {
        if (this.#timer !== null)
            clearTimeout(this.#timer);
        this.#timer = null;
        const value = this.#pending;
        this.#pending = null;
        if (value === null || !this.#alive)
            return;
        try {
            this.surface.changeOpacity(value / 100);
        }
        catch (error) {
            this.report(error);
        }
    }
}
/** The field label for one loader, matching multi-loader node conventions. */
export function maskOpacityLabel(field) {
    return field
        ? field + ' ' + WORKFLOW_SURFACE_COPY.visualOpacity
        : WORKFLOW_SURFACE_COPY.visualOpacity;
}
