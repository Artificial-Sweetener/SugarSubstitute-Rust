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
 * Where the masonry must be scrolled to land the same distance into the kept scroll's Cube, or
 * null while that Cube's section is not laid out.
 */
export function masonryScrollTop(element, scroll) {
    if (scroll.cube === null)
        return scroll.offset;
    const section = masonryCubeSections(element).find((item) => item.id === scroll.cube);
    return section ? section.y + scroll.offset : null;
}
