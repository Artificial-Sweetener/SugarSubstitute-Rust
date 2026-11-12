/** Resolve only the exact current catalog item rather than reusing a batch index across generations. */
export function canvasMediaItem(snapshot, target) {
    const item = canvasMediaFile(snapshot, target);
    return item?.mediaKind === 'image' ? item : null;
}
/** Resolve an exact ready final of any supported kind for its original-file action. */
export function canvasMediaFile(snapshot, target) {
    const item = target === 'primary'
        ? !snapshot.preview &&
            snapshot.view.mode !== 'overview' &&
            snapshot.resource.state === 'ready'
            ? snapshot.selected
            : null
        : target === 'secondary'
            ? snapshot.secondaryResource?.state === 'ready'
                ? snapshot.secondary
                : null
            : (snapshot.grid
                .find((entry) => entry.source.id === target.sourceId)
                ?.items.find((item) => item.id === target.itemId) ?? null);
    return item && item.mediaKind !== 'unsupported' && !item.unavailable ? item : null;
}
/** List every still image of a result's sources in catalog order, excluding live previews. */
export function canvasMediaImages(entries) {
    return entries.flatMap((entry) => entry.items.filter((item) => item.mediaKind === 'image'));
}
