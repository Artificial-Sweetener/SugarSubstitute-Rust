import { planWorkflowSurfaces, workflowSyncSurfaces, } from '../domain/WorkflowSurfacePlan.js';
import { WORKFLOW_SURFACE_COPY } from './WorkflowSurfaceCopy.js';
/**
 * Plan surfaces from graph facts and replace the canvas's surface set for the active workflow.
 *
 * Change notifications coalesce into at most one sync in flight; a change during a sync schedules
 * exactly one follow-up. An unchanged plan is not resent. After a successful sync every provisioned
 * mask loader is bound to its mask layer, except loaders whose user chose another source.
 */
export class WorkflowSurfaceProvisioning {
    graph;
    host;
    accent;
    files;
    notice;
    report;
    #listeners = new Set();
    #stop;
    #sizes = new Map();
    #rejectedInitials = new Set();
    #state = null;
    #signature = null;
    #running = null;
    #dirty = false;
    #disposed = false;
    /** Observe the graph and resolve the canvas lazily so a reconnected canvas is picked up. */
    constructor(graph, host, accent, files, notice, report) {
        this.graph = graph;
        this.host = host;
        this.accent = accent;
        this.files = files;
        this.notice = notice;
        this.report = report;
        this.#stop = graph.subscribe(() => this.request());
    }
    /** The last successfully synced surfaces, or null when no canvas holds them. */
    get state() {
        return this.#state;
    }
    /** Observe synced-state changes. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Name a provisioned document so sources bound to it can be captured while it is inactive. */
    document(canvasId, documentId) {
        const state = this.#state;
        if (state?.canvasId !== canvasId)
            return null;
        return state.surfaces.find((surface) => surface.document.id === documentId)?.document ?? null;
    }
    /** Schedule a coalesced sync after the current task's graph changes settle. */
    request() {
        if (this.#disposed)
            return Promise.resolve();
        this.#dirty = true;
        this.#running ??= Promise.resolve()
            .then(() => this.#drain())
            .finally(() => {
            this.#running = null;
        });
        return this.#running;
    }
    /** Resend the current plan even if unchanged, after the canvas reconnects or the workflow loads. */
    refresh() {
        this.#signature = null;
        return this.request();
    }
    /** Replace a synced surface after the canvas resized it, keeping every other surface. */
    replaceSurface(key, document) {
        const state = this.#state;
        if (!state)
            return;
        this.#publish({
            ...state,
            surfaces: state.surfaces.map((surface) => surface.key === key ? { ...surface, document } : surface),
        });
    }
    /** Stop observing and drop state; a sync in flight finishes without publishing. */
    dispose() {
        this.#disposed = true;
        this.#stop();
        this.#listeners.clear();
        this.#state = null;
    }
    /** Sync until no change arrived during the previous sync. */
    async #drain() {
        while (this.#dirty && !this.#disposed) {
            this.#dirty = false;
            try {
                await this.#sync();
            }
            catch (error) {
                this.report(error);
            }
        }
    }
    /** Plan, verify initial mask files, sync when changed, then bind provisioned loaders. */
    async #sync() {
        const host = this.host();
        const planned = host
            ? planWorkflowSurfaces(this.graph.facts(), this.accent(), {
                synthetic: WORKFLOW_SURFACE_COPY.maskOnlySurface,
            })
            : null;
        const workflow = planned ? this.graph.workflow(planned.surfaces.length > 0) : null;
        if (!host || !planned || !workflow) {
            this.#signature = null;
            if (this.#state)
                this.#publish(null);
            return;
        }
        const plan = await this.#verified(planned);
        if (this.#disposed)
            return;
        const surfaces = workflowSyncSurfaces(plan);
        const signature = JSON.stringify([host.canvasId, workflow, surfaces]);
        if (signature === this.#signature)
            return;
        await this.#followAuthoritySizes(host, workflow, plan);
        if (this.#disposed)
            return;
        const result = await host.execute({ type: 'sync', workflow, surfaces });
        if (this.#disposed)
            return;
        if (result.kind === 'failure') {
            this.#signature = null;
            if (result.status !== 'cancelled' && result.status !== 'unavailable')
                throw new Error(result.message || WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
            return;
        }
        if (result.kind !== 'synced')
            throw new Error(WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
        const synced = new Map(result.surfaces.map((surface) => [surface.key, surface]));
        const provisioned = plan.surfaces.map((surface) => {
            const reply = synced.get(surface.key);
            const layers = new Map(reply?.masks.map((mask) => [mask.key, mask.layerId]));
            const masks = surface.masks.map((mask) => {
                const layerId = layers.get(mask.key);
                if (!layerId)
                    throw new Error(WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
                return { ...mask, layerId };
            });
            if (!reply)
                throw new Error(WORKFLOW_SURFACE_COPY.surfaceSyncFailed);
            return { ...surface, document: reply.document, masks };
        });
        this.#signature = signature;
        this.#publish({
            canvasId: host.canvasId,
            workflow,
            surfaces: provisioned,
            rejected: plan.rejected,
        });
        this.#bind(host.canvasId, provisioned);
    }
    /**
     * Resize existing mask-only surfaces whose size fields were edited directly, keeping their
     * pixels centred, so a sync never has to change the size of a surface the canvas holds.
     */
    async #followAuthoritySizes(host, workflow, plan) {
        const state = this.#state;
        if (state?.workflow !== workflow || state.canvasId !== host.canvasId)
            return;
        for (const surface of plan.surfaces) {
            const current = state.surfaces.find((item) => item.key === surface.key)?.document;
            if (surface.base.kind !== 'blank' ||
                !current ||
                (current.width === surface.base.width && current.height === surface.base.height))
                continue;
            const result = await host.execute({
                type: 'resize',
                workflow,
                surface: surface.key,
                width: surface.base.width,
                height: surface.base.height,
                mode: { kind: 'canvas', anchor: 'center' },
            });
            if (result.kind === 'failure') {
                if (result.status !== 'cancelled')
                    throw new Error(result.message);
                continue;
            }
            const document = result.kind === 'synced'
                ? result.surfaces.find((item) => item.key === surface.key)?.document
                : undefined;
            if (document)
                this.replaceSurface(surface.key, document);
        }
    }
    /** Bind each mask loader's entries to their layers through the loader's own binding owner. */
    #bind(canvasId, surfaces) {
        const endpoints = new Map();
        for (const surface of surfaces)
            for (const mask of surface.masks) {
                const sources = endpoints.get(mask.endpoint) ?? new Map();
                sources.set(mask.entry, {
                    kind: 'canvas',
                    canvasId,
                    documentId: surface.document.id,
                    target: { kind: 'layer', layerId: mask.layerId },
                });
                endpoints.set(mask.endpoint, sources);
            }
        for (const [endpoint, sources] of endpoints)
            try {
                this.graph.binding(endpoint)?.provision(sources);
            }
            catch (error) {
                this.report(error);
            }
    }
    /**
     * Drop initial mask files whose size differs from their surface.
     *
     * A file whose size cannot be read is also dropped: a mask is only ever seeded from a file
     * that is known to match its image. The user is told once when they pick such a file for a
     * mask that already exists; a loader's preexisting file is skipped silently when its mask is
     * first created, so the mask simply starts blank.
     */
    async #verified(plan) {
        const existing = new Set(this.#state?.surfaces.flatMap((surface) => surface.masks.map((mask) => mask.key)));
        const surfaces = await Promise.all(plan.surfaces.map(async (surface) => {
            if (!surface.masks.some((mask) => mask.initial))
                return surface;
            const size = surface.base.kind === 'blank'
                ? { width: surface.base.width, height: surface.base.height }
                : await this.#size(surface.base.file);
            const masks = await Promise.all(surface.masks.map(async (mask) => {
                if (!mask.initial)
                    return mask;
                const initial = await this.#size(mask.initial);
                if (initial && size && initial.width === size.width && initial.height === size.height)
                    return mask;
                const key = JSON.stringify([mask.key, mask.initial, size]);
                if (existing.has(mask.key) && !this.#rejectedInitials.has(key)) {
                    this.#rejectedInitials.add(key);
                    if (initial && size)
                        this.notice(WORKFLOW_SURFACE_COPY.maskDimensionsMismatchTitle, WORKFLOW_SURFACE_COPY.maskDimensionsMismatch(`${String(initial.width)}x${String(initial.height)}`, `${String(size.width)}x${String(size.height)}`));
                    else
                        this.notice(WORKFLOW_SURFACE_COPY.maskDimensionsUnverifiedTitle, WORKFLOW_SURFACE_COPY.maskDimensionsUnverified);
                }
                return { ...mask, initial: null };
            }));
            return { ...surface, masks };
        }));
        return { ...plan, surfaces };
    }
    /** Read each file's size once per page; a failed read is retried on the next sync. */
    #size(file) {
        const key = JSON.stringify(file);
        let size = this.#sizes.get(key);
        if (!size) {
            size = this.files.read(file).catch((error) => {
                this.#sizes.delete(key);
                this.report(error);
                return null;
            });
            this.#sizes.set(key, size);
        }
        return size;
    }
    /** Publish a new state to observers. */
    #publish(state) {
        this.#state = state;
        for (const listener of this.#listeners)
            try {
                listener();
            }
            catch (error) {
                this.report(error);
            }
    }
}
