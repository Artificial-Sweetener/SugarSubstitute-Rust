//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Lay out, scroll and select overview cells independently of how their images are painted. */
import { canvasOverviewPlacements, canvasOverviewWindow, } from '../application/CanvasOverviewWindow.js';
import { ensureCanvasOverviewStyles } from './CanvasOverviewStyles.js';
/** Keep only the visible window's controls, batching scroll and resize bursts per frame. */
export class CanvasOverviewGrid {
    options;
    element;
    #track;
    #window;
    #resize;
    #cells = new Map();
    #catalog = null;
    #frame = null;
    #active = false;
    #signature = '';
    #disposed = false;
    #columns = null;
    #reference = null;
    /** Mount a hidden region under the attachment root without accessing the Comfy host. */
    constructor(parent, options) {
        this.options = options;
        const document = parent.ownerDocument;
        ensureCanvasOverviewStyles(document);
        this.element = document.createElement('div');
        this.element.className = 'sc-output-overview';
        this.element.tabIndex = 0;
        this.element.setAttribute('role', 'region');
        this.element.setAttribute('aria-label', 'Workflow output overview');
        this.element.hidden = true;
        this.#track = document.createElement('div');
        this.#track.className = 'sc-output-overview-track';
        this.#window = document.createElement('div');
        this.#window.className = 'sc-output-overview-window';
        this.#track.append(this.#window);
        this.element.append(this.#track);
        parent.append(this.element);
        this.element.addEventListener('scroll', this.#schedule);
        const Observer = document.defaultView?.ResizeObserver;
        this.#resize = Observer ? new Observer(this.#schedule) : null;
        this.#resize?.observe(this.element);
    }
    /** Report whether the grid currently presents overview mode. */
    get active() {
        return this.#active && !this.#disposed;
    }
    /** Expose the rendered cells' controls in window order for painter-specific hit testing. */
    get cellElements() {
        return [...this.#cells.values()];
    }
    /** Refresh light catalog descriptors, retaining controls whose logical cells remain visible. */
    reconcile(catalog, active) {
        const entered = active && !this.#active;
        this.#catalog = catalog;
        this.#active = active;
        this.element.hidden = !active;
        if (active)
            this.render(entered);
        else {
            this.#frame?.();
            this.#frame = null;
            this.#signature = '';
            this.#columns = null;
            this.#reference = null;
            this.options.painter.clear();
            this.#cells.clear();
            this.#window.replaceChildren();
        }
    }
    /** Project a pending source's visible cells without replacing the current gallery. */
    cells(catalog) {
        const bounds = this.element.getBoundingClientRect();
        return canvasOverviewWindow(catalog, bounds.width, bounds.height, this.element.scrollTop, catalog.entries[0]?.items[0]?.dimensions ?? this.#reference, this.#columns).cells;
    }
    /** Lay out the visible window; `force` re-announces visible cells after suspension or entry. */
    render(force) {
        if (!this.#catalog || !this.#active || this.#disposed)
            return;
        const bounds = this.element.getBoundingClientRect();
        this.#reference =
            this.#catalog.entries[0]?.preview?.dimensions ??
                this.#catalog.entries[0]?.items[0]?.dimensions ??
                this.options.painter.reference() ??
                this.#reference;
        const window = canvasOverviewWindow(this.#catalog, bounds.width, bounds.height, this.element.scrollTop, this.#reference, this.#columns);
        const placements = canvasOverviewPlacements(window);
        this.#columns = window.columns;
        this.#track.style.height = `${window.totalHeight}px`;
        this.#window.style.top = `${window.offset}px`;
        this.#window.style.left = `${window.left}px`;
        this.#window.style.width = `${window.width}px`;
        this.#window.style.gridTemplateColumns = `repeat(${window.columns},minmax(0,1fr))`;
        this.#window.style.gridAutoRows = `${window.cellHeight}px`;
        this.#window.style.gap = `${window.gap}px`;
        const keys = new Set(window.cells.map((cell) => cell.key));
        for (const [key, elements] of this.#cells)
            if (!keys.has(key)) {
                elements.button.remove();
                this.#cells.delete(key);
            }
        const rendered = [];
        for (const [index, cell] of window.cells.entries()) {
            let elements = this.#cells.get(cell.key);
            if (!elements ||
                elements.cell.item?.id !== cell.item?.id ||
                elements.cell.label !== cell.label ||
                elements.cell.preview !== cell.preview) {
                const replacement = this.#cell(cell);
                if (elements)
                    elements.button.replaceWith(replacement.button);
                elements = replacement;
                this.#cells.set(cell.key, elements);
            }
            if (this.#window.children[index] !== elements.button)
                this.#window.insertBefore(elements.button, this.#window.children[index] ?? null);
            const shift = placements[index]?.shift ?? 0;
            elements.button.style.transform = shift ? `translateX(${shift}px)` : '';
            rendered.push(elements);
        }
        const signature = JSON.stringify(window.cells.map((cell) => [cell.key, cell.item?.id, cell.preview]));
        if (force || signature !== this.#signature) {
            this.#signature = signature;
            this.options.visible?.(window.cells);
        }
        this.options.painter.paint(rendered, placements, bounds);
    }
    /** Stop finite scroll/resize work and detach this grid's controls. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#frame?.();
        this.#frame = null;
        this.#resize?.disconnect();
        this.element.removeEventListener('scroll', this.#schedule);
        this.options.painter.clear();
        this.#cells.clear();
        this.element.remove();
    }
    /** Coalesce a scroll/resize burst into one layout and paint pass, with no idle work. */
    #schedule = () => {
        if (this.#frame || !this.#active || this.#disposed)
            return;
        this.#frame = this.options.frames.request(() => {
            this.#frame = null;
            this.render(false);
        });
    };
    /** Keep source descriptions accessible without placing captions over the batch images. */
    #cell(cell) {
        const document = this.element.ownerDocument;
        const button = document.createElement('button');
        button.type = 'button';
        button.className = 'sc-output-overview-cell';
        button.setAttribute('aria-label', cell.label);
        const image = document.createElement('div');
        image.className = 'sc-output-overview-image';
        const status = document.createElement('span');
        status.className = 'sc-output-overview-cell-status';
        button.append(image, status);
        if (cell.item && cell.item.mediaKind !== 'image' && cell.item.mediaKind !== 'unsupported') {
            const badge = document.createElement('span');
            badge.className = 'sc-output-media-badge';
            badge.textContent =
                cell.item.mediaKind === 'model'
                    ? '3D'
                    : cell.item.mediaKind === 'video'
                        ? 'Video'
                        : 'Animation';
            button.append(badge);
        }
        button.addEventListener('click', () => this.options.select(cell));
        return { cell, button, image, status };
    }
}
