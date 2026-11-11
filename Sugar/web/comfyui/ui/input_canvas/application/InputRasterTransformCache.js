import { MAX_CANVAS_RASTER_BYTES } from '../../core/embedded/CanvasRuntime.js';
/** Identify transformations by actual pixels and parameters rather than transient capture objects. */
export class InputRasterTransformCache {
    budget;
    limit;
    #ready = [];
    #pending = new Set();
    #bytes = 0;
    #epoch = 0;
    /** Keep cache limits injectable for deterministic eviction and admission proofs. */
    constructor(budget = MAX_CANVAS_RASTER_BYTES, limit = 16) {
        this.budget = budget;
        this.limit = limit;
    }
    /** Share pending work and reuse completed work only when every input pixel and parameter matches. */
    read(sources, parameters, create, resultBytes) {
        const matches = (entry) => entry.parameters === parameters &&
            entry.sources.length === sources.length &&
            entry.sources.every((source, index) => {
                const candidate = sources[index];
                return (candidate !== undefined &&
                    source.width === candidate.width &&
                    source.height === candidate.height &&
                    source.format === candidate.format &&
                    source.data === candidate.data);
            });
        const readyIndex = this.#ready.findIndex(matches);
        if (readyIndex >= 0) {
            const ready = this.#ready.splice(readyIndex, 1)[0];
            if (ready) {
                this.#ready.push(ready);
                return ready.promise;
            }
        }
        for (const pending of this.#pending)
            if (matches(pending))
                return pending.promise;
        if (this.#pending.size >= this.limit)
            return Promise.reject(new Error('Too many pending raster transformations.'));
        const epoch = this.#epoch;
        let work;
        try {
            work = create();
        }
        catch (error) {
            return Promise.reject(error instanceof Error
                ? error
                : new Error('Raster transformation failed.', { cause: error }));
        }
        const entry = {
            sources: sources.map((source) => Object.freeze({ ...source })),
            parameters,
            bytes: sources.reduce((sum, source) => sum + source.data.length * 2, 0),
            promise: work
                .then((value) => {
                entry.bytes += resultBytes(value);
                if (epoch === this.#epoch && entry.bytes <= this.budget) {
                    this.#ready.push(entry);
                    this.#bytes += entry.bytes;
                    while (this.#ready.length > this.limit || this.#bytes > this.budget) {
                        const oldest = this.#ready.shift();
                        if (oldest)
                            this.#bytes -= oldest.bytes;
                    }
                }
                return value;
            })
                .finally(() => this.#pending.delete(entry)),
        };
        this.#pending.add(entry);
        return entry.promise;
    }
    /** Retire retained pixels and fence late completions when their owning session ends. */
    clear() {
        this.#epoch++;
        this.#ready.length = 0;
        this.#bytes = 0;
        this.#pending.clear();
    }
}
