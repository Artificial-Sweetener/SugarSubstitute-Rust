import { WORKFLOW_SURFACE_COPY } from './WorkflowSurfaceCopy.js';
/** Inclusive pixel range offered for a mask-only canvas. */
export const CANVAS_RESOLUTION_LIMITS = Object.freeze({ minimum: 64, maximum: 32768, step: 8 });
/**
 * Resize the canvas first and write the workflow only after the canvas accepted the new size,
 * so the workflow never names a size its canvas does not have.
 */
export class WorkflowCanvasResolution {
    surfaces;
    host;
    writer;
    /** Address surfaces through the canvas that holds them. */
    constructor(surfaces, host, writer) {
        this.surfaces = surfaces;
        this.host = host;
        this.writer = writer;
    }
    /** Find the mask-only surface that a node's width or height field sizes. */
    surface(node, field) {
        return (this.surfaces.state?.surfaces.find((surface) => surface.authority?.some((item) => item.node === node && (item.width === field || item.height === field))) ?? null);
    }
    /** Find the mask-only surface that any size field of a node sizes. */
    surfaceSizedBy(node) {
        return (this.surfaces.state?.surfaces.find((surface) => surface.authority?.some((item) => item.node === node)) ?? null);
    }
    /** Resize one mask-only surface; failures leave the workflow unchanged and explain why. */
    async resize(key, width, height, mode) {
        for (const value of [width, height])
            if (!Number.isSafeInteger(value) ||
                value < CANVAS_RESOLUTION_LIMITS.minimum ||
                value > CANVAS_RESOLUTION_LIMITS.maximum)
                throw new RangeError(WORKFLOW_SURFACE_COPY.resizeInvalid);
        const state = this.surfaces.state, host = this.host();
        const surface = state?.surfaces.find((item) => item.key === key);
        if (!state || !host || host.canvasId !== state.canvasId || !surface?.authority)
            throw new Error(WORKFLOW_SURFACE_COPY.resizeStale);
        if (surface.document.width === width && surface.document.height === height)
            return;
        const result = await host.execute({
            type: 'resize',
            workflow: state.workflow,
            surface: key,
            width,
            height,
            mode,
        });
        const synced = result.kind === 'synced' ? result.surfaces.find((item) => item.key === key) : undefined;
        if (!synced)
            throw new Error(WORKFLOW_SURFACE_COPY.resizeFailed);
        this.surfaces.replaceSurface(key, synced.document);
        try {
            this.writer.write(surface.authority, width, height);
        }
        catch (error) {
            throw new Error(WORKFLOW_SURFACE_COPY.resizeWriteFailed, { cause: error });
        }
    }
}
