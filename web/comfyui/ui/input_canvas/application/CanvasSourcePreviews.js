import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import { MAX_CANVAS_RASTER_BYTES, canvasInputSourceKey, } from '../../core/embedded/CanvasRuntime.js';
/** Own preview coalescing and subscription lifetime independently of any mask interpretation or view. */
export class CanvasSourcePreviews {
    capture;
    host;
    report;
    workflows;
    #entries = new Map();
    #hosts = new Map();
    #dirty = new Set();
    #cohorts = new WeakMap();
    #nextCohort = 0;
    #pending = false;
    #disposed = false;
    #workflow = null;
    /**
     * Reuse the same validated capture path as full-resolution queue preparation.
     *
     * A workflow canvas numbers each workflow's input documents and layers afresh, so the same
     * source names different pixels in another workflow. Each preview is keyed by the workflow
     * loaded when it was observed, is captured only while the canvas holds that workflow and
     * shows loading until then; a result arriving after the canvas moved on is dropped. The owner
     * calls `refresh` once the canvas holds another workflow, which captures the waiting previews.
     */
    constructor(capture, host, report, workflows) {
        this.capture = capture;
        this.host = host;
        this.report = report;
        this.workflows = workflows;
    }
    /** Share raw source pixels so a channel change can reinterpret them without a native round trip. */
    observe(source, listener, maximumSize = 288, representation = 'raw', cohort = null) {
        if (this.#disposed)
            return () => { };
        let cohortId = 0;
        if (cohort) {
            cohortId = this.#cohorts.get(cohort) ?? ++this.#nextCohort;
            this.#cohorts.set(cohort, cohortId);
        }
        const workflow = this.workflows.loaded();
        const key = JSON.stringify([
            workflow,
            cohortId,
            canvasInputSourceKey(source),
            maximumSize === 0 ? 'full' : 'display',
            representation,
        ]);
        let entry = this.#entries.get(key);
        if (!entry) {
            entry = {
                workflow,
                cohort: cohortId,
                source,
                maximumSize,
                representation,
                listeners: new Set(),
                value: { kind: 'loading' },
                serial: 0,
            };
            this.#entries.set(key, entry);
        }
        // Keep the peak while mounted so smaller consumers can reuse a larger completed capture.
        if (maximumSize > entry.maximumSize) {
            entry.maximumSize = maximumSize;
            this.#invalidate(key, entry);
        }
        entry.listeners.add(listener);
        listener(entry.value);
        const host = this.host(source.canvasId);
        if (host && !this.#hosts.has(host)) {
            const lease = host.changes?.subscribe(() => this.refresh(host.canvasId));
            this.#hosts.set(host, lease ? () => lease.stop() : () => { });
        }
        if (entry.value.kind === 'loading')
            this.#invalidate(key, entry);
        this.#schedule();
        return () => {
            entry.listeners.delete(listener);
            if (!entry.listeners.size) {
                this.#entries.delete(key);
                this.#dirty.delete(key);
            }
            for (const [host, stop] of this.#hosts)
                if (![...this.#entries.values()].some((value) => value.source.canvasId === host.canvasId)) {
                    stop();
                    this.#hosts.delete(host);
                }
        };
    }
    /**
     * Drop every captured preview once another workflow is loaded.
     *
     * Mounted previews show loading and capture again; previews observed from now on are keyed by
     * the new workflow. An unchanged workflow is a no-op.
     */
    workflowChanged() {
        const workflow = this.workflows.loaded();
        if (this.#disposed || workflow === this.#workflow)
            return;
        this.#workflow = workflow;
        for (const [key, entry] of this.#entries) {
            entry.value = { kind: 'loading' };
            for (const listener of entry.listeners)
                listener(entry.value);
            this.#invalidate(key, entry);
        }
        this.#schedule();
    }
    /** Mark only the changed canvas's mounted sources dirty without graph discovery or file uploads. */
    refresh(canvasId) {
        for (const [key, entry] of this.#entries)
            if (entry.source.canvasId === canvasId)
                this.#invalidate(key, entry);
        this.#schedule();
    }
    /** Refresh sources used by one picker while unrelated mounted inputs retain their previews. */
    refreshSources(sources) {
        const requested = new Set(sources.filter((source) => source.kind === 'canvas').map(canvasInputSourceKey));
        for (const [key, entry] of this.#entries)
            if (requested.has(canvasInputSourceKey(entry.source)))
                this.#invalidate(key, entry);
        this.#schedule();
    }
    /** Stop accepting results before releasing every native metadata subscription. */
    dispose() {
        this.#disposed = true;
        for (const stop of this.#hosts.values())
            stop();
        this.#hosts.clear();
        this.#entries.clear();
        this.#dirty.clear();
    }
    /** Whether the canvas holds the documents of the workflow this preview was observed in. */
    #held(entry) {
        return this.workflows.holds(entry.source.canvasId, entry.workflow);
    }
    /** Fence in-flight results whenever a later committed edit supersedes their native revision. */
    #invalidate(key, entry) {
        entry.serial++;
        this.#dirty.add(key);
        if (entry.cohort)
            for (const [peerKey, peer] of this.#entries)
                if (peer !== entry && peer.cohort === entry.cohort) {
                    peer.serial++;
                    this.#dirty.add(peerKey);
                }
    }
    /** Admit one coalesced read at a time and keep changes arriving during capture queued. */
    #schedule() {
        if (this.#pending || this.#disposed || !this.#dirty.size)
            return;
        this.#pending = true;
        void Promise.resolve()
            .then(() => this.#read())
            .finally(() => {
            this.#pending = false;
            this.#schedule();
        });
    }
    /** Preserve per-canvas failure isolation and suppress obsolete results after unmount or a later edit. */
    async #read() {
        const requested = [...this.#dirty];
        this.#dirty.clear();
        const groups = new Map();
        for (const key of requested) {
            const entry = this.#entries.get(key);
            // Waits, showing loading, until the owner refreshes the canvas holding its workflow.
            if (!entry || !this.#held(entry))
                continue;
            const groupKey = JSON.stringify([entry.source.canvasId, entry.maximumSize, entry.cohort]);
            const group = groups.get(groupKey) ?? [];
            group.push({ key, entry, serial: entry.serial });
            groups.set(groupKey, group);
        }
        for (const group of groups.values()) {
            const size = group[0]?.entry.maximumSize ?? 0;
            const count = group[0]?.entry.cohort
                ? group.length
                : size
                    ? Math.max(1, Math.min(64, Math.floor(MAX_CANVAS_RASTER_BYTES / (size * size * 5))))
                    : 1;
            for (let offset = 0; offset < group.length; offset += count) {
                let pending = group.slice(offset, offset + count);
                while (pending.length) {
                    const batch = pending;
                    pending = [];
                    try {
                        const snapshots = await this.capture.capture(batch.map(({ key, entry }) => ({
                            key,
                            source: entry.source,
                            representation: entry.representation,
                        })), batch[0]?.entry.maximumSize ?? 288);
                        for (const request of batch) {
                            const snapshot = snapshots.get(request.key);
                            if (!snapshot)
                                throw new Error('Missing canvas source preview.');
                            this.#publish(request, {
                                kind: 'pixels',
                                pixels: snapshot.pixels,
                                document: snapshot.document,
                                revision: snapshot.revision,
                            });
                        }
                    }
                    catch (error) {
                        if (this.#disposed ||
                            !batch.some((request) => this.#entries.get(request.key) === request.entry &&
                                request.serial === request.entry.serial &&
                                this.#held(request.entry)))
                            continue;
                        const unavailable = error instanceof CanvasSourceUnavailable;
                        if (!unavailable)
                            this.report(error);
                        const sourceKey = unavailable && error.source ? canvasInputSourceKey(error.source) : null;
                        const affected = sourceKey
                            ? batch.filter((request) => canvasInputSourceKey(request.entry.source) === sourceKey)
                            : batch;
                        if (unavailable && affected.length)
                            pending = batch.filter((request) => !affected.includes(request));
                        for (const request of affected)
                            this.#publish(request, {
                                kind: 'error',
                                unavailable,
                                message: error instanceof Error ? error.message : 'Canvas source preview unavailable.',
                            });
                    }
                }
            }
        }
    }
    /** Publish only to the exact still-mounted source generation that requested this snapshot. */
    #publish(request, value) {
        const { key, entry, serial } = request;
        if (this.#disposed ||
            this.#entries.get(key) !== entry ||
            entry.serial !== serial ||
            !this.#held(entry))
            return;
        entry.value = value;
        for (const listener of entry.listeners)
            listener(value);
    }
}
