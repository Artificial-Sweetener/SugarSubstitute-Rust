import { BudgetedMemoryCache } from '../../core/memory/BudgetedMemoryCache.js';
/**
 * Bound cached encoded posters independently of the canvas's decoded-pixel budget.
 *
 * Posters are encoded data, so they are the last tier the page allowance evicts; their own
 * count and byte bounds still apply.
 */
export class BrowserCanvasMediaPosters {
    renderer;
    #cache;
    #pending = new Map();
    #disposed = false;
    /** Inject the actual browser/host capture boundary and the page memory allowance. */
    constructor(renderer, memory) {
        this.renderer = renderer;
        this.#cache = new BudgetedMemoryCache({
            id: 'output-canvas-posters',
            registry: memory,
            maxEntries: 24,
            maxBytes: 8 * 1024 * 1024,
        });
    }
    /** Reuse an exact revision's poster and never cache a retired request. */
    async read(item, url, signal) {
        if (signal.aborted)
            throw signal.reason;
        if (this.#disposed)
            throw new Error('Media preview cache is disposed.');
        const key = JSON.stringify([
            item.provenance.sessionId,
            item.provenance.generation,
            item.provenance.runOrder,
            item.provenance.revision,
            item.file,
        ]);
        const cached = this.#cache.get(key);
        if (cached)
            return cached;
        let capture = this.#pending.get(key);
        if (!capture || capture.controller.signal.aborted) {
            const controller = new AbortController();
            const consumers = new Set();
            const completion = Promise.resolve()
                .then(() => {
                if (controller.signal.aborted)
                    throw controller.signal.reason;
                return this.renderer.render(item, url, controller.signal);
            })
                .then((blob) => {
                if (controller.signal.aborted)
                    throw controller.signal.reason;
                this.#store(key, blob);
                return blob;
            })
                .finally(() => {
                if (this.#pending.get(key)?.controller === controller)
                    this.#pending.delete(key);
            });
            capture = { controller, consumers, completion };
            this.#pending.set(key, capture);
        }
        return this.#subscribe(capture, signal);
    }
    /** Expose explicit encoded-preview accounting without recurring telemetry. */
    diagnostics() {
        return { posters: this.#cache.size, posterBytes: this.#cache.bytes() };
    }
    /** Release encoded previews with the owning canvas host. */
    dispose() {
        this.#disposed = true;
        for (const capture of this.#pending.values())
            capture.controller.abort();
        this.#pending.clear();
        this.#cache.dispose();
    }
    /** Keep shared capture alive until its last visible/admitted consumer retires. */
    #subscribe(capture, signal) {
        const consumer = Symbol();
        capture.consumers.add(consumer);
        return new Promise((resolve, reject) => {
            let settled = false;
            const release = () => {
                if (settled)
                    return false;
                settled = true;
                signal.removeEventListener('abort', aborted);
                capture.controller.signal.removeEventListener('abort', aborted);
                capture.consumers.delete(consumer);
                if (!capture.consumers.size)
                    capture.controller.abort();
                return true;
            };
            const aborted = () => {
                if (release())
                    reject(new DOMException('Media preview request canceled.', 'AbortError'));
            };
            signal.addEventListener('abort', aborted, { once: true });
            capture.controller.signal.addEventListener('abort', aborted, { once: true });
            void capture.completion.then((blob) => {
                if (release())
                    resolve(blob);
            }, (error) => {
                if (release())
                    reject(error instanceof Error ? error : new Error('Media preview capture failed.'));
            });
        });
    }
    /** Retain encoded previews only within explicit count and byte limits. */
    #store(key, blob) {
        if (this.#disposed || blob.size > 1024 * 1024)
            return;
        this.#cache.set(key, blob, { tier: 3, bytes: blob.size });
    }
}
