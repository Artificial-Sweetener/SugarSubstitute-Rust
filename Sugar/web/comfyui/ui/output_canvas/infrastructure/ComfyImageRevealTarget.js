import { canvasRevealTarget } from '../domain/CanvasReveal.js';
/** Match files within one source; ignore thumbnail encoding and cache-busting parameters. */
export function resolveComfyImageReveal(catalog, path, value, origin, batchIndex) {
    if (!catalog || !path)
        return null;
    let url;
    try {
        url = new URL(value, origin);
    }
    catch {
        return null;
    }
    if (url.origin !== origin || !['/view', '/api/view'].includes(url.pathname))
        return null;
    const source = catalog.sources.find((source) => source.executionPath === path);
    const entry = source
        ? catalog.result.outputs.find((output) => output.source.id === source.id)
        : null;
    const matches = entry?.items.filter((item) => item.mediaKind === 'image' &&
        item.file.filename === url.searchParams.get('filename') &&
        item.file.subfolder === (url.searchParams.get('subfolder') ?? '') &&
        item.file.type === (url.searchParams.get('type') ?? 'output'));
    const item = matches?.length === 1
        ? matches[0]
        : batchIndex === null
            ? null
            : matches?.find((item) => item.batchIndex === batchIndex);
    return item ? canvasRevealTarget(item) : null;
}
