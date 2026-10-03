import { resolveMasonryNodeIcon } from './MasonryNodeIconPolicy.js';
const iconFiles = {
    application: 'app-generic.svg',
    edit: 'edit.svg',
    eraser: 'eraser.svg',
    folder: 'folder.svg',
    model: 'brain-circuit.svg',
    palette: 'color.svg',
    photo: 'image.svg',
};
/** Reattach a decorative glyph when Vue replaces its title, preserving native content/events. */
export class MasonryNodeIconHost {
    #cell;
    #node;
    #icon;
    #observer;
    #title = null;
    #disposed = false;
    /** Bind only to an explicitly marked masonry cell, independent of the appearance theme. */
    constructor(cell, node) {
        this.#cell = cell;
        this.#node = node;
        this.#icon = cell.ownerDocument.createElement('span');
        this.#icon.className = 'sugarcubes-workflow-masonry__node-icon';
        this.#icon.setAttribute('aria-hidden', 'true');
        this.#observer = new MutationObserver(() => this.refresh());
        this.#observer.observe(cell, { childList: true, subtree: true });
        this.refresh();
    }
    /** Resolve the current exact header and update only SugarCubes-owned icon state. */
    refresh() {
        if (this.#disposed)
            return;
        const icon = this.#cell.hasAttribute('data-masonry-card')
            ? resolveMasonryNodeIcon(this.#node)
            : null;
        const title = [...this.#cell.querySelectorAll('[data-testid="node-title"]')].find((candidate) => candidate.closest('[data-masonry-card]') === this.#cell &&
            candidate.closest('[data-testid^="node-header-"]')?.getAttribute('data-testid') ===
                `node-header-${String(this.#node.id ?? '')}`) ?? null;
        if (this.#title !== title || !icon)
            this.#releaseTitle();
        if (!title || !icon)
            return;
        this.#title = title;
        title.dataset.masonryTitleIcon = '';
        if (this.#icon.dataset.masonryNodeIcon !== icon) {
            this.#icon.dataset.masonryNodeIcon = icon;
            const url = new URL(`../../../../icons/node/${iconFiles[icon]}`, import.meta.url);
            this.#icon.style.setProperty('--sc-node-icon', `url("${url.href}")`);
        }
        if (this.#icon.parentElement !== title)
            title.prepend(this.#icon);
    }
    /** Stop observing before the native card is released, including detached-title markers. */
    dispose() {
        this.#disposed = true;
        this.#observer.disconnect();
        this.#releaseTitle();
    }
    /** Remove only our decorative element and its title-layout marker. */
    #releaseTitle() {
        if (this.#title)
            delete this.#title.dataset.masonryTitleIcon;
        this.#icon.remove();
        this.#title = null;
    }
}
