/** Index current finals once so camera capture remains linear in retained scene membership. */
export function canvasInspectionCatalog(catalog) {
    const sources = new Set(catalog.sources.map((source) => source.id));
    const slots = new Map();
    const collect = (item, scene) => {
        if (!sources.has(item.provenance.sourceId))
            return;
        slots.set(item.id, Object.freeze({
            sourceId: item.provenance.sourceId,
            batchIndex: item.batchIndex,
            ...(scene ? { scene } : {}),
        }));
    };
    for (const output of catalog.result.outputs)
        for (const item of output.items)
            collect(item);
    const group = catalog.result.scenes;
    if (group)
        for (const scene of group.scenes) {
            const scope = Object.freeze([group.id, scene.key]);
            for (const output of scene.outputs)
                for (const item of output.items)
                    collect(item, scope);
        }
    return slots;
}
/** Qualify one final without inferring scene membership from its source, filename or current tab. */
export function canvasInspectionSlot(selection, item, catalog) {
    return ((item ? canvasInspectionCatalog(catalog).get(item.id) : null) ?? Object.freeze({ ...selection }));
}
/** Retain every current scene's finals, including inactive scenes outside the projected catalog. */
export function canvasInspectionSlots(catalog) {
    return [...canvasInspectionCatalog(catalog).values()];
}
