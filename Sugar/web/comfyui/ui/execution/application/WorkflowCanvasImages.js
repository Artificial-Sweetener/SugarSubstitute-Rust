/** Own stable per-source batch positions and replay suppression for one captured run. */
export class WorkflowCanvasImages {
    #seen = new Set();
    #positions = new Map();
    /** Admit each finished image immediately, retaining its mask-batch position across incremental events. */
    read(run, source, images) {
        const fresh = [];
        for (const media of images) {
            if (!media || media.mediaKind !== 'image' || media.file.type === 'input')
                continue;
            const id = JSON.stringify([
                run.session.sessionId,
                run.session.generation,
                run.promptId,
                source,
                media.file.type,
                media.file.subfolder,
                media.file.filename,
            ]);
            if (this.#seen.has(id))
                continue;
            this.#seen.add(id);
            const batchIndex = this.#positions.get(source) ?? 0;
            this.#positions.set(source, batchIndex + 1);
            fresh.push(Object.freeze({ id, media, batchIndex }));
        }
        return Object.freeze(fresh);
    }
}
