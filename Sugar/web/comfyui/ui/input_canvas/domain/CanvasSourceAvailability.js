/** Preserve source identity so one unavailable target does not invalidate other previews. */
export class CanvasSourceUnavailable extends Error {
    source;
    /** Keep actionable, concise feedback separate from unexpected capture faults. */
    constructor(message, source = null) {
        super(message);
        this.source = source;
        this.name = 'CanvasSourceUnavailable';
    }
}
/** Explain missing source prerequisites without requesting native pixels or changing authored intent. */
export function canvasSourceUnavailable(source, context) {
    if (source.kind === 'file')
        return source.file ? null : 'Choose a file';
    if (!context?.supported)
        return 'Canvas unavailable';
    if (!context.document)
        return 'Open a document';
    if (source.documentId !== null && source.documentId !== context.document.id)
        return 'Open the source document';
    if (!context.targets.includes(source.target.kind))
        return 'Source unavailable';
    const target = source.target;
    if (target.kind === 'current-layer' &&
        (!context.currentLayerId ||
            !context.layers.some((layer) => layer.id === context.currentLayerId)))
        return 'Select a layer';
    if ((target.kind === 'layer' || target.kind === 'composition-from-layer') &&
        !context.layers.some((layer) => layer.id === target.layerId))
        return 'Choose a layer';
    return null;
}
