import { canvasSessionId } from './CanvasIdentity.js';
/** Capture immutable provenance when the thumbnail is presented, never when it is clicked. */
export function canvasRevealTarget(item) {
    return Object.freeze({
        sessionId: item.provenance.sessionId,
        generation: item.provenance.generation,
        sourceId: item.provenance.sourceId,
        runOrder: item.provenance.runOrder,
        itemId: item.id,
        batchIndex: item.batchIndex,
    });
}
/** Refuse superseded runs and files rather than substituting the current image in the same slot. */
export function resolveCanvasReveal(catalog, target) {
    if (catalog.sessionId !== canvasSessionId(target.sessionId) ||
        catalog.generation !== target.generation)
        return null;
    if (!catalog.sources.some((source) => source.id === target.sourceId))
        return null;
    const item = catalog.result.outputs
        .find((entry) => entry.source.id === target.sourceId)
        ?.items.find((candidate) => candidate.id === target.itemId && candidate.batchIndex === target.batchIndex);
    return item?.mediaKind === 'image' &&
        item.provenance.sourceId === target.sourceId &&
        item.provenance.runOrder === target.runOrder &&
        item.provenance.sessionId === target.sessionId &&
        item.provenance.generation === target.generation
        ? item
        : null;
}
