import { readInputBinding } from './InputBindingMetadata.js';
import { inputLabel } from '../domain/InputLoader.js';
import { defaultCanvasInput } from '../domain/CanvasInputSource.js';
/** Use the same default policy for loaders queued before masonry has mounted their controls. */
export class ComfyInputSourceDefaults {
    host;
    legacy;
    /** Resolve native capabilities and existing persisted values at their adapter boundary. */
    constructor(host, legacy) {
        this.host = host;
        this.legacy = legacy;
    }
    /** Preserve explicit choices and prefer current selection only for unconfigured capable canvases. */
    async resolve(properties, loader, files) {
        const saved = readInputBinding(properties, loader.kind, loader.field) ??
            (loader.kind === 'mask' ? await this.legacy.read(properties) : null);
        if (saved)
            return saved;
        const canvas = this.host();
        if (!canvas)
            return null;
        const context = await canvas.execute({ type: 'context' });
        if (context.kind === 'failure')
            throw new Error(context.message);
        if (context.kind !== 'context' ||
            !context.supported ||
            !context.targets.includes(loader.kind === 'mask' ? 'selection' : 'composition'))
            return null;
        const entries = (loader.ordered && files.length ? files : [files[0] ?? null]).map((file, index) => ({
            id: `source-${String(index + 1)}`,
            label: `${inputLabel(loader.kind)} ${String(index + 1)}`,
            source: defaultCanvasInput(canvas.canvasId, context, file, loader.kind === 'mask' ? 'selection' : 'composition'),
        }));
        return { version: 2, field: loader.field, files, entries, selected: entries[0]?.id ?? null };
    }
}
