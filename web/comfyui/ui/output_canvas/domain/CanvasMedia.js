/** Resolve only the exact current catalog item rather than reusing a batch index across generations. */
export function canvasMediaItem(snapshot, target) {
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
    return item?.mediaKind === 'image' ? item : null;
}
