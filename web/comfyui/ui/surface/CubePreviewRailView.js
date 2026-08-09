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
/** Render Cube output previews in one rail. */
import { CUBE_PREVIEW_ITEM_GAP, resolveCubeOutputSections, resolveCubePreviewItemGrid, } from './CubePreviewSections.js';
/** Own safe preview media DOM for one Cube face. */
export class CubePreviewRailView {
    element;
    #actions;
    #resizeObserver;
    #signature = '';
    /** Bind one dedicated rail without owning preview collection. */
    constructor(element, actions = null) {
        this.element = element;
        this.#actions = actions;
        const ResizeObserverConstructor = element.ownerDocument.defaultView?.ResizeObserver;
        this.#resizeObserver = ResizeObserverConstructor
            ? new ResizeObserverConstructor(() => this.reflow())
            : null;
    }
    /** Render one immutable preview snapshot without output-selection state. */
    render(snapshot) {
        const signature = JSON.stringify(snapshot);
        if (signature === this.#signature)
            return;
        this.#signature = signature;
        const documentRef = this.element.ownerDocument;
        const media = documentRef.createElement('div');
        media.className = 'sugarcubes-cube-face__preview-media';
        const outputs = resolveCubeOutputSections(snapshot);
        const outputGrid = documentRef.createElement('div');
        outputGrid.className = 'sugarcubes-cube-face__preview-outputs';
        outputGrid.dataset.cubePreviewOutputs = '';
        outputGrid.style.gridTemplateRows = `repeat(${outputs.length}, minmax(0, 1fr))`;
        for (const output of outputs) {
            const section = documentRef.createElement('section');
            section.className = 'sugarcubes-cube-face__preview-output';
            section.dataset.cubePreviewOutput = output.id;
            section.setAttribute('aria-label', `Preview for ${output.label}`);
            const title = documentRef.createElement('header');
            title.className = 'sugarcubes-cube-face__preview-output-title';
            title.dataset.cubePreviewOutputTitle = '';
            const label = documentRef.createElement('span');
            label.dataset.cubePreviewOutputLabel = '';
            label.textContent = output.id;
            title.append(label);
            section.append(title);
            if (output.items.length > 0) {
                const itemGrid = documentRef.createElement('div');
                itemGrid.className = 'sugarcubes-cube-face__preview-items';
                itemGrid.dataset.cubePreviewItems = output.id;
                itemGrid.style.gap = `${String(CUBE_PREVIEW_ITEM_GAP)}px`;
                for (const item of output.items) {
                    itemGrid.append(buildPreviewFigure(documentRef, item, this.#actions));
                }
                section.append(itemGrid);
            }
            else {
                const empty = documentRef.createElement('p');
                empty.textContent = 'No preview available';
                section.append(empty);
            }
            outputGrid.append(section);
        }
        if (outputs.length === 0) {
            const empty = documentRef.createElement('p');
            empty.textContent = 'No preview available';
            media.append(empty);
        }
        else {
            media.append(outputGrid);
        }
        this.element.replaceChildren(media);
        this.#observeItemGrids();
        this.reflow();
    }
    /** Recompute every item grid from its currently rendered dimensions. */
    reflow() {
        for (const itemGrid of this.element.querySelectorAll('[data-cube-preview-items]')) {
            const bounds = itemGrid.getBoundingClientRect();
            const grid = resolveCubePreviewItemGrid(bounds, itemGrid.childElementCount);
            const columns = `repeat(${String(grid.columns)}, minmax(0, 1fr))`;
            const rows = `repeat(${String(grid.rows)}, minmax(0, 1fr))`;
            if (itemGrid.style.gridTemplateColumns !== columns) {
                itemGrid.style.gridTemplateColumns = columns;
            }
            if (itemGrid.style.gridTemplateRows !== rows) {
                itemGrid.style.gridTemplateRows = rows;
            }
        }
    }
    /** Release responsive layout observation owned by this rail. */
    dispose() {
        this.#resizeObserver?.disconnect();
    }
    /** Observe the current immutable snapshot's item-grid elements. */
    #observeItemGrids() {
        this.#resizeObserver?.disconnect();
        for (const itemGrid of this.element.querySelectorAll('[data-cube-preview-items]')) {
            this.#resizeObserver?.observe(itemGrid);
        }
    }
}
/** Build one safe media figure without parsing dynamic host values as markup. */
function buildPreviewFigure(documentRef, item, actions) {
    const figure = documentRef.createElement('figure');
    figure.dataset.cubePreviewItem = item.key;
    figure.setAttribute('aria-label', item.label);
    const image = documentRef.createElement('img');
    image.src = item.url;
    image.alt = '';
    image.loading = 'eager';
    figure.append(image);
    if (actions) {
        figure.addEventListener('contextmenu', (event) => actions.openContextMenu(item, event));
        const download = documentRef.createElement('button');
        download.type = 'button';
        download.className = 'sugarcubes-cube-face__preview-download';
        download.dataset.cubePreviewDownload = item.key;
        download.setAttribute('aria-label', 'Download image');
        download.title = 'Download image';
        const icon = documentRef.createElement('i');
        icon.classList.add('pi', 'pi-download');
        icon.setAttribute('aria-hidden', 'true');
        download.append(icon);
        download.addEventListener('pointerdown', (event) => event.stopPropagation());
        download.addEventListener('click', (event) => {
            event.preventDefault();
            event.stopPropagation();
            actions.download(item);
        });
        figure.append(download);
    }
    return figure;
}
