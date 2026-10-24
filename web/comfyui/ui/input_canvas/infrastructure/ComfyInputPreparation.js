import { inputWorkflowProjection } from './InputWorkflowProjection.js';
/** Prepare both queue routes without changing graph topology or authored file values. */
export class ComfyInputPreparation {
    loaders;
    sources;
    defaults;
    /** Share the exact catalog and capture owner used by masonry bindings. */
    constructor(loaders, sources, defaults) {
        this.loaders = loaders;
        this.sources = sources;
        this.defaults = defaults;
    }
    /** Pin entry identity, order and channel before any asynchronous native capture or upload. */
    async prepare(payload) {
        const projection = await inputWorkflowProjection(payload, this.loaders, (properties, loader, files) => this.defaults.resolve(properties, loader, files));
        const entries = projection.targets.flatMap((target) => target.binding.entries.flatMap((entry) => entry.source.kind === 'canvas'
            ? [
                {
                    key: JSON.stringify([target.key, entry.id]),
                    source: entry.source,
                    representation: target.loader.kind === 'image' ? 'image' : 'raw',
                },
            ]
            : []));
        if (!projection.targets.length)
            return payload;
        const files = await this.sources.prepare(entries);
        for (const target of projection.targets)
            target.apply(target.binding.entries.map((entry) => {
                const file = entry.source.kind === 'canvas'
                    ? files.get(JSON.stringify([target.key, entry.id]))
                    : entry.source.file;
                if (!file)
                    throw new Error('Choose a file or canvas source for every entry.');
                return file;
            }));
        return projection.payload;
    }
}
