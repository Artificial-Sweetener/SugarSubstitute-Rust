import { maskColor, regionMaskColor } from './MaskColorScheme.js';
/** At most this many surfaces cross the wire in one sync, per the canvas contract. */
export const MAX_WORKFLOW_SURFACES = 64;
/** At most this many masks belong to one surface, per the canvas contract. */
export const MAX_SURFACE_MASKS = 32;
/**
 * Plan one surface per image endpoint and one per shared mask size authority.
 *
 * A mask paired with an image joins that image's surface. An unpaired mask joins the mask-only
 * surface of its resolved size authority, so masks sized by the same nodes share one surface.
 * Ambiguous pairings and unresolved sizes are rejected rather than guessed. Scalar mask colours
 * rotate over each surface's complete mask list, starting from the accent; the regions of an
 * ordered loader take the regional palette over that loader's own regions, as their prompts do.
 */
export function planWorkflowSurfaces(facts, accent, labels) {
    const rejected = [];
    const authored = new Map(facts.images.flatMap((image) => image.file ? [[image.key, { image, masks: [] }]] : []));
    const synthetic = new Map();
    for (const mask of facts.masks) {
        if (mask.image === 'ambiguous') {
            rejected.push({ endpoint: mask.key, reason: 'ambiguous_image' });
            continue;
        }
        if (mask.image !== null) {
            authored.get(mask.image)?.masks.push(mask);
            continue;
        }
        const authority = mask.authority;
        if (authority.kind !== 'resolved') {
            rejected.push({
                endpoint: mask.key,
                reason: authority.kind === 'ambiguous' ? 'ambiguous_size' : 'unsized',
            });
            continue;
        }
        const group = synthetic.get(authority.key) ?? { authority, masks: [] };
        group.masks.push(mask);
        synthetic.set(authority.key, group);
    }
    const surfaces = [];
    const place = (surface, masks) => {
        if (surfaces.length >= MAX_WORKFLOW_SURFACES) {
            for (const mask of masks)
                rejected.push({ endpoint: mask.key, reason: 'limit' });
            return;
        }
        const entries = masks.flatMap((mask) => mask.entries.map((entry, index) => ({
            mask,
            entry,
            label: mask.regions?.names[index] ?? entry.label,
            region: mask.regions ? regionMaskColor(index, mask.entries.length) : null,
        })));
        const admitted = entries.slice(0, MAX_SURFACE_MASKS);
        for (const mask of new Set(entries.slice(MAX_SURFACE_MASKS).map(({ mask }) => mask.key)))
            rejected.push({ endpoint: mask, reason: 'limit' });
        surfaces.push({
            ...surface,
            masks: admitted.map(({ mask, entry, label, region }, index) => ({
                key: workflowMaskKey(mask.key, entry.entry),
                endpoint: mask.key,
                entry: entry.entry,
                label,
                color: region ?? maskColor(accent, index, admitted.length),
                opacity: mask.opacity,
                initial: entry.initial,
                channel: mask.channel,
            })),
        });
    };
    for (const { image, masks } of authored.values())
        if (image.file)
            place({
                key: image.key,
                label: image.label,
                image: image.key,
                authority: null,
                base: { kind: 'file', file: image.file },
            }, masks);
    for (const { authority, masks } of synthetic.values())
        place({
            key: '@synthetic/' + stableHash(authority.key),
            label: masks[0]?.label ?? labels.synthetic,
            image: null,
            authority: authority.nodes,
            base: { kind: 'blank', width: authority.width, height: authority.height },
        }, masks);
    return { surfaces, rejected };
}
/** Key a scalar mask by its loader field and a region by its persisted entry identity. */
export function workflowMaskKey(endpoint, entry) {
    return entry === null ? endpoint : JSON.stringify([endpoint, entry]);
}
/** Project a plan onto the `workflow.sync` surface list. */
export function workflowSyncSurfaces(plan) {
    return plan.surfaces.map((surface) => ({
        key: surface.key,
        label: surface.label,
        base: surface.base,
        masks: surface.masks.map((mask) => ({
            key: mask.key,
            label: mask.label,
            color: mask.color,
            opacity: mask.opacity,
            initial: mask.initial,
            channel: mask.channel,
        })),
    }));
}
/** Parse a Comfy loader value such as `sub/dir/name.png [output]` into a file reference. */
export function comfyFileReference(value) {
    const match = /^(.*?)(?: \[(input|output|temp)\])?$/.exec(value.trim());
    const path = (match?.[1] ?? '').replaceAll('\\', '/');
    const parts = path.split('/'), filename = parts.pop() ?? '';
    if (!filename || parts.some((part) => part === '..'))
        return null;
    const type = match?.[2];
    return {
        filename,
        subfolder: parts.join('/'),
        type: type === 'output' || type === 'temp' ? type : 'input',
    };
}
/** Shorten a structural identity into a fixed-width key with 64-bit FNV-1a over UTF-16 units. */
function stableHash(value) {
    let hash = 0xcbf29ce484222325n;
    for (let index = 0; index < value.length; index++) {
        const unit = value.charCodeAt(index);
        for (const byte of [unit & 0xff, unit >> 8]) {
            hash ^= BigInt(byte);
            hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
        }
    }
    return hash.toString(16).padStart(16, '0');
}
