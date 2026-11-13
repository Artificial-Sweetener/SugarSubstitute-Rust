import { maskSourceImage } from '../domain/MaskSourcePixels.js';
import { readInputBinding } from './InputBindingMetadata.js';
import { readLegacyMaskBinding } from './LegacyMaskBinding.js';
/** Keep compatibility capture outside the current source-targeting application model. */
export class LegacyMaskMigration {
    host;
    assets;
    /** Borrow legacy native support only for existing saved references and use the normal source uploader. */
    constructor(host, assets) {
        this.host = host;
        this.assets = assets;
    }
    /** Convert actual stored coverage without substituting a former filename or the current selection. */
    async read(properties) {
        const current = readInputBinding(properties, 'mask');
        if (current)
            return current;
        const legacy = readLegacyMaskBinding(properties);
        if (!legacy)
            return null;
        const groups = new Map();
        for (const entry of legacy.entries)
            if (entry.mask) {
                const key = JSON.stringify([entry.mask.canvasId, entry.mask.document]);
                const group = groups.get(key) ?? [];
                if (!group.some((mask) => mask.id === entry.mask?.id))
                    group.push(entry.mask);
                groups.set(key, group);
            }
        const files = new Map();
        for (const masks of groups.values()) {
            const first = masks[0];
            if (!first)
                continue;
            const host = this.host(first.canvasId);
            if (!host)
                throw new Error('Connect the canvas containing the saved mask before loading this workflow.');
            const result = await host.execute({ type: 'capture', masks, maximumSize: 0 });
            if (result.kind === 'failure')
                throw new Error(result.message);
            if (result.kind !== 'captured' ||
                result.snapshots.length !== masks.length ||
                new Set(result.snapshots.map((snapshot) => snapshot.revision)).size > 1)
                throw new Error('The saved canvas masks could not be captured coherently.');
            for (const mask of masks) {
                const values = result.snapshots.filter((snapshot) => JSON.stringify(snapshot.mask) === JSON.stringify(mask)), snapshot = values[0];
                if (values.length !== 1 ||
                    !snapshot ||
                    snapshot.pixels.width !== mask.document.width ||
                    snapshot.pixels.height !== mask.document.height)
                    throw new Error('The saved canvas mask identity or geometry changed.');
                const image = maskSourceImage({ ...snapshot.pixels, format: 'coverage8' });
                files.set(JSON.stringify(mask), await this.assets.stage(image));
            }
        }
        return {
            version: 2,
            field: legacy.field,
            files: legacy.files,
            selected: legacy.selected,
            entries: legacy.entries.map((entry) => ({
                id: entry.id,
                label: entry.label,
                source: {
                    kind: 'file',
                    file: entry.mask ? (files.get(JSON.stringify(entry.mask)) ?? null) : entry.file,
                },
            })),
        };
    }
}
