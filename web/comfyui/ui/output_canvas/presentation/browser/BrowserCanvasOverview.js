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
/** Present packed batch images without owning image loading or result history. */
import { canvasOverviewWindow, } from '../../application/CanvasOverviewWindow.js';
/** Keep only the visible window's controls and adopt already-admitted image handles. */
export class BrowserCanvasOverview {
    frames;
    visible;
    select;
    element;
    #track;
    #window;
    #resize;
    #cells = new Map();
    #catalog = null;
    #images = [];
    #frame = null;
    #active = false;
    #signature = '';
    #disposed = false;
    #columns = null;
    #reference = null;
    /** Bind explicit visible-interest and navigation collaborators without accessing the Comfy host. */
    constructor(parent, frames, visible, select) {
        this.frames = frames;
        this.visible = visible;
        this.select = select;
        const document = parent.ownerDocument;
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
    /** Refresh light catalog descriptors, retaining controls whose logical cells remain visible. */
    reconcile(catalog, active) {
        const entered = active && !this.#active;
        this.#catalog = catalog;
        this.#active = active;
        this.element.hidden = !active;
        if (active)
            this.#render(entered);
        else {
            this.#frame?.();
            this.#frame = null;
            this.#signature = '';
            this.#columns = null;
            this.#reference = null;
            this.#images = [];
            this.#cells.clear();
            this.#window.replaceChildren();
        }
    }
    /** Re-measure a settled allocation and restore interests after resource suspension. */
    resize() {
        if (this.#active && !this.#disposed)
            this.#render(true);
    }
    /** Project a pending source's visible cells without replacing the current gallery. */
    cells(catalog) {
        const bounds = this.element.getBoundingClientRect();
        return canvasOverviewWindow(catalog, bounds.width, bounds.height, this.element.scrollTop, catalog.entries[0]?.items[0]?.dimensions ?? this.#reference, this.#columns).cells;
    }
    /** Attach resource-owned handles without copying pixels, URLs, or decode authority. */
    images(images) {
        this.#images = images;
        if (this.#active)
            this.#render(false);
    }
    /** Qualify the exact rendered batch item without admitting latent previews or pending replacements. */
    draggable(target) {
        if (!this.#active || this.#disposed)
            return null;
        for (const [key, elements] of this.#cells) {
            const frame = this.#images.find((image) => image.slot === key)?.frame;
            if (frame &&
                !frame.preview &&
                target === frame.handle &&
                frame.key === elements.cell.item?.id)
                return frame;
        }
        return null;
    }
    /** Stop finite scroll/resize work and detach this gallery's controls and image references. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#frame?.();
        this.#frame = null;
        this.#resize?.disconnect();
        this.element.removeEventListener('scroll', this.#schedule);
        this.#images = [];
        this.#cells.clear();
        this.element.remove();
    }
    /** Coalesce a scroll/resize burst into one layout and interest update, with no idle work. */
    #schedule = () => {
        if (this.#frame || !this.#active || this.#disposed)
            return;
        this.#frame = this.frames.request(() => {
            this.#frame = null;
            this.#render(false);
        });
    };
    /** Project at most eight visible cells before notifying the resource admission owner. */
    #render(force) {
        if (!this.#catalog || !this.#active || this.#disposed)
            return;
        const bounds = this.element.getBoundingClientRect();
        this.#reference =
            this.#catalog.entries[0]?.preview?.dimensions ??
                this.#catalog.entries[0]?.items[0]?.dimensions ??
                this.#images.find((image) => image.frame)?.frame?.dimensions ??
                this.#reference;
        const window = canvasOverviewWindow(this.#catalog, bounds.width, bounds.height, this.element.scrollTop, this.#reference, this.#columns);
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
            const finalCount = window.count % window.columns;
            const finalRow = window.count <= 8 && finalCount > 0 && index >= window.count - finalCount;
            const shift = finalRow
                ? ((window.columns - finalCount) * (window.width + window.gap)) / window.columns / 2
                : 0;
            elements.button.style.transform = shift ? `translateX(${shift}px)` : '';
        }
        const signature = JSON.stringify(window.cells.map((cell) => [cell.key, cell.item?.id, cell.preview]));
        if (force || signature !== this.#signature) {
            this.#signature = signature;
            this.visible(window.cells);
        }
        this.#present();
    }
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
        button.addEventListener('click', () => this.select(cell));
        return { cell, button, image, status };
    }
    /** Preserve resource failure/budget explanations instead of pretending every cell has a thumbnail. */
    #present() {
        for (const [key, elements] of this.#cells) {
            const snapshot = this.#images.find((image) => image.slot === key);
            const handle = snapshot?.frame?.handle;
            if (handle && elements.image.firstChild !== handle) {
                handle.className = 'sc-output-overview-thumbnail';
                handle.alt = '';
                elements.image.replaceChildren(handle);
            }
            else if (!handle)
                elements.image.replaceChildren();
            elements.status.textContent = handle
                ? ''
                : snapshot?.status.state === 'error' || snapshot?.status.state === 'budget'
                    ? snapshot.status.message
                    : elements.cell.preview
                        ? ''
                        : !elements.cell.item
                            ? 'This batch image is unavailable.'
                            : elements.cell.item.mediaKind !== 'image'
                                ? 'Select to open this media.'
                                : '';
        }
    }
}
