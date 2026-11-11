/** Preserve one authoritative ordering and labeling for reusable source controls. */
export const CANVAS_INPUT_CHOICES = [
    { kind: 'selection', label: 'Canvas Selection' },
    { kind: 'current-layer', label: 'Current Layer' },
    { kind: 'layer', label: 'Specific Layer' },
    { kind: 'composition', label: 'Current Composition' },
    { kind: 'composition-from-layer', label: 'Current Comp From Layer' },
    { kind: 'file', label: 'Load From File' },
];
/** Bind explicit layers to their document while leaving current targets responsive to active context. */
export function chooseCanvasInput(canvasId, context, target) {
    if (target.kind === 'file')
        return target;
    if (!canvasId || !context?.supported || !context.targets.includes(target.kind))
        throw new Error('The connected canvas does not support this source.');
    const explicit = target.kind === 'layer' || target.kind === 'composition-from-layer';
    if (explicit &&
        (!context.document || !context.layers.some((layer) => layer.id === target.layerId)))
        throw new Error('Choose an available layer in the connected canvas.');
    return {
        kind: 'canvas',
        canvasId,
        documentId: explicit ? (context.document?.id ?? null) : null,
        target,
    };
}
/** Prefer current selection only when that operation is advertised by the connected canvas. */
export function defaultCanvasInput(canvasId, context, file, preferred) {
    return context?.supported && context.targets.includes(preferred)
        ? chooseCanvasInput(canvasId, context, { kind: preferred })
        : { kind: 'file', file };
}
