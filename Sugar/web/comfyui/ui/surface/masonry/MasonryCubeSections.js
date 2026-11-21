const CUBE_SECTION = 'cube:';
/** The Cube sections the masonry shows, top to bottom, at their settled layout positions. */
export function masonryCubeSections(element) {
    return [...element.querySelectorAll('header[data-masonry-section]')]
        .flatMap((header) => {
        const section = header.dataset.masonrySection ?? '';
        if (!section.startsWith(CUBE_SECTION) || header.hasAttribute('data-sc-search-card-hidden'))
            return [];
        const y = Number(header.dataset.masonryY) + (header.parentElement?.offsetTop ?? 0);
        const height = Number(header.dataset.masonryHeight);
        return Number.isFinite(y) && Number.isFinite(height) && height >= 0
            ? [{ id: section.slice(CUBE_SECTION.length), y, height }]
            : [];
    })
        .sort((a, b) => a.y - b.y);
}
/** Where the masonry is scrolled: the Cube whose section holds its top, and how far into it. */
export function captureMasonryScroll(element) {
    const top = Math.max(0, element.scrollTop);
    const holding = masonryCubeSections(element)
        .filter((section) => section.y <= top)
        .at(-1);
    return holding
        ? { cube: holding.id, offset: Math.round(top - holding.y) }
        : { cube: null, offset: Math.round(top) };
}
/**
 * Scroll the masonry back to a kept scroll: the same distance into the same Cube, or the top when
 * that Cube is gone.
 */
export function restoreMasonryScroll(element, scroll) {
    if (!scroll) {
        element.scrollTop = 0;
        return;
    }
    if (scroll.cube === null) {
        element.scrollTop = scroll.offset;
        return;
    }
    const section = masonryCubeSections(element).find((item) => item.id === scroll.cube);
    element.scrollTop = section ? section.y + scroll.offset : 0;
}
