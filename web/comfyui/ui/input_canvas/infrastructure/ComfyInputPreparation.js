import { inputWorkflowProjection } from './InputWorkflowProjection.js';
/** Prepare both queue routes without changing graph topology or authored file values. */
export class ComfyInputPreparation {
    loaders;
    sources;
    defaults;
    returns;
    /** Share the exact catalog and capture owner used by masonry bindings. */
    constructor(loaders, sources, defaults, returns) {
        this.loaders = loaders;
        this.sources = sources;
        this.defaults = defaults;
        this.returns = returns;
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
            return { payload };
        const ports = this.loaders.assetPorts();
        const origin = await this.returns.origin(projection.targets);
        const sources = await this.sources.prepare(entries);
        const canvas = await this.returns.prepare(projection.targets, sources, ports, origin);
        for (const target of projection.targets)
            target.apply(target.binding.entries.map((entry) => {
                const file = entry.source.kind === 'canvas'
                    ? sources.files.get(JSON.stringify([target.key, entry.id]))
                    : entry.source.file;
                if (!file)
                    throw new Error('Choose a file or canvas source for every entry.');
                return file;
            }));
        return { payload: projection.payload, ...(canvas ? { canvas } : {}) };
    }
}
