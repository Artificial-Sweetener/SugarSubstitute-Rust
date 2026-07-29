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
/** Keep the Cube editor HUD clear of Comfy's native viewport chrome as it changes. */
const LEFT_CHROME_SELECTOR = [
    '.side-tool-bar-container',
    '.side-bar-panel',
    '[class*="side-bar" i]',
    '[class*="sidebar" i]',
].join(', ');
const TOP_CHROME_SELECTOR = '.subgraph-breadcrumb';
const VIEWPORT_GUTTER_PX = 16;
/** Own the DOM observation required to keep one viewport overlay out of Comfy's chrome. */
export class CubeEditorHudPositioner {
    #document;
    #root;
    #mutationObserver;
    #resizeObserver;
    #onWindowResize;
    #queued = false;
    /** Start tracking the native viewport chrome for one mounted HUD. */
    constructor(documentRef, root) {
        this.#document = documentRef;
        this.#root = root;
        this.#onWindowResize = () => this.#schedule();
        const ResizeObserverConstructor = documentRef.defaultView?.ResizeObserver;
        this.#resizeObserver = ResizeObserverConstructor
            ? new ResizeObserverConstructor(() => this.#schedule())
            : null;
        this.#mutationObserver = new MutationObserver(() => this.#schedule());
        this.#mutationObserver.observe(documentRef.documentElement, {
            attributes: true,
            attributeFilter: ['class', 'style'],
            childList: true,
            subtree: true,
        });
        documentRef.defaultView?.addEventListener('resize', this.#onWindowResize);
        this.#observeChrome();
        this.sync();
    }
    /** Release document observers when the HUD leaves the editor. */
    dispose() {
        this.#mutationObserver.disconnect();
        this.#resizeObserver?.disconnect();
        this.#document.defaultView?.removeEventListener('resize', this.#onWindowResize);
    }
    /** Write viewport insets consumed by the HUD's native-token stylesheet. */
    sync() {
        const left = occupiedLeftEdge(this.#document);
        const top = occupiedTopEdge(this.#document);
        this.#root.style.setProperty('--sugarcubes-cube-editor-metadata-left', `${String(Math.max(VIEWPORT_GUTTER_PX, Math.ceil(left + VIEWPORT_GUTTER_PX)))}px`);
        this.#root.style.setProperty('--sugarcubes-cube-editor-metadata-top', `${String(Math.max(VIEWPORT_GUTTER_PX, Math.ceil(top + VIEWPORT_GUTTER_PX)))}px`);
    }
    /** Batch native-chrome DOM changes into one layout read. */
    #schedule() {
        if (this.#queued)
            return;
        this.#queued = true;
        queueMicrotask(() => {
            this.#queued = false;
            this.#observeChrome();
            this.sync();
        });
    }
    /** Observe the geometry of Comfy chrome that can change without a DOM mutation. */
    #observeChrome() {
        if (!this.#resizeObserver)
            return;
        for (const element of chromeElements(this.#document))
            this.#resizeObserver.observe(element);
    }
}
/** Return the rightmost edge claimed by Comfy's permanent toolbar or expanded panel. */
function occupiedLeftEdge(documentRef) {
    const permanentToolbarRight = occupiedPermanentToolbarEdge(documentRef);
    return chromeElements(documentRef)
        .filter((element) => isVisibleLeftChrome(element, permanentToolbarRight))
        .reduce((rightmost, element) => Math.max(rightmost, element.getBoundingClientRect().right), 0);
}
/** Find Comfy's left-docked chrome across native and extension-provided side panels. */
function chromeElements(documentRef) {
    return Array.from(documentRef.querySelectorAll(LEFT_CHROME_SELECTOR));
}
/** Return the permanent rail edge that an expanded Comfy panel docks against. */
function occupiedPermanentToolbarEdge(documentRef) {
    return Array.from(documentRef.querySelectorAll('.side-tool-bar-container'))
        .filter(isVisible)
        .reduce((rightmost, element) => Math.max(rightmost, element.getBoundingClientRect().right), 0);
}
/** Return the bottom edge claimed by the native Cube-editor breadcrumb. */
function occupiedTopEdge(documentRef) {
    const breadcrumb = documentRef.querySelector(TOP_CHROME_SELECTOR);
    return breadcrumb && isVisible(breadcrumb) ? breadcrumb.getBoundingClientRect().bottom : 0;
}
/** Restrict left offsets to visible chrome docked on the viewport's left edge. */
function isVisibleLeftChrome(element, permanentToolbarRight) {
    return element.getBoundingClientRect().left <= permanentToolbarRight + 1 && isVisible(element);
}
/** Ignore retained or hidden Comfy chrome nodes. */
function isVisible(element) {
    const rect = element.getBoundingClientRect();
    const style = element.ownerDocument.defaultView?.getComputedStyle(element);
    return (rect.width > 0 && rect.height > 0 && style?.display !== 'none' && style?.visibility !== 'hidden');
}
