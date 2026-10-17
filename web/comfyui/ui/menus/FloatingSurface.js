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
/** Own detached, bounded popup geometry and lifecycle without participating in application layout. */
import { applyOwnedTheme } from '../surface/appearance/ThemeService.js';
import { readComfyDarkTheme } from '../surface/appearance/ComfyColorScheme.js';
/** Keep all popup kinds within the viewport while preferring their natural opening direction. */
export function floatingPosition(anchor, width, height, viewportWidth, viewportHeight) {
    const gap = 4;
    let left = anchor.kind === 'pointer' ? anchor.x : anchor.bounds.left;
    let top = anchor.kind === 'pointer' ? anchor.y : anchor.bounds.bottom + gap;
    if (anchor.kind === 'submenu') {
        left = anchor.bounds.right - gap;
        top = anchor.bounds.top;
        if (left + width > viewportWidth - gap)
            left = anchor.bounds.left - width + gap;
    }
    else if (anchor.kind === 'below' && top + height > viewportHeight - gap)
        top = anchor.bounds.top - height - gap;
    return {
        left: Math.max(gap, Math.min(left, viewportWidth - width - gap)),
        top: Math.max(gap, Math.min(top, viewportHeight - height - gap)),
    };
}
/** Own one top-layer surface, including a fixed body portal when popovers are unavailable. */
export class FloatingSurface {
    #document;
    element;
    #options = null;
    #observer = null;
    /** Bind a feature-owned content element to the shared floating boundary. */
    constructor(element) {
        this.element = element;
        this.#document = element.ownerDocument;
        element.dataset.scFloatingSurface = '';
        element.setAttribute('popover', 'manual');
        // Geometry is an invariant even before styles load or when feature styles are absent.
        for (const [name, value] of Object.entries({
            position: 'fixed',
            inset: 'auto',
            margin: '0',
            'box-sizing': 'border-box',
            'max-width': 'calc(100vw - 8px)',
            'max-height': 'calc(100vh - 8px)',
            'z-index': '100000',
        }))
            element.style.setProperty(name, value, 'important');
    }
    /** Attach content outside every feature container and position it before accepting interaction. */
    open(options) {
        this.close();
        this.#options = options;
        this.#document.body.append(this.element);
        applyOwnedTheme(this.element, readComfyDarkTheme(this.#document));
        if (typeof this.element.showPopover === 'function')
            this.element.showPopover();
        this.reposition();
        const windowRef = this.#document.defaultView;
        if (windowRef?.ResizeObserver) {
            this.#observer = new windowRef.ResizeObserver(() => this.reposition());
            this.#observer.observe(this.element);
        }
        this.#document.addEventListener('pointerdown', this.#outside, true);
        this.#document.addEventListener('scroll', this.#scroll, true);
        windowRef?.addEventListener('resize', this.#dismiss);
        this.element.addEventListener('pointerdown', this.#stop);
        this.element.addEventListener('wheel', this.#stop);
        this.element.addEventListener('contextmenu', this.#context);
    }
    /** Reconcile geometry when popup content changes without moving surrounding content. */
    reposition() {
        if (!this.#options)
            return;
        const windowRef = this.#document.defaultView;
        this.element.style.setProperty('max-height', this.#options.maxHeight === undefined
            ? 'calc(100vh - 8px)'
            : `${Math.max(0, Math.min(this.#options.maxHeight, (windowRef?.innerHeight ?? this.#document.documentElement.clientHeight) - 8))}px`, 'important');
        const rect = this.element.getBoundingClientRect();
        const position = floatingPosition(this.#options.anchor, rect.width, rect.height, windowRef?.innerWidth ?? this.#document.documentElement.clientWidth, windowRef?.innerHeight ?? this.#document.documentElement.clientHeight);
        this.element.style.setProperty('left', `${String(position.left)}px`, 'important');
        this.element.style.setProperty('top', `${String(position.top)}px`, 'important');
    }
    /** Release all observers, listeners and top-layer state without invoking feature callbacks. */
    close() {
        this.#observer?.disconnect();
        this.#observer = null;
        if (this.element.isConnected && typeof this.element.hidePopover === 'function')
            this.element.hidePopover();
        applyOwnedTheme(this.element, null);
        this.element.remove();
        this.#document.removeEventListener('pointerdown', this.#outside, true);
        this.#document.removeEventListener('scroll', this.#scroll, true);
        this.#document.defaultView?.removeEventListener('resize', this.#dismiss);
        this.element.removeEventListener('pointerdown', this.#stop);
        this.element.removeEventListener('wheel', this.#stop);
        this.element.removeEventListener('contextmenu', this.#context);
        this.#options = null;
    }
    /** Dismiss only interactions outside the complete owning popup tree. */
    #outside = (event) => {
        if (event.target instanceof Node && !this.#contains(event.target))
            this.#dismiss();
    };
    /** Keep internal scrolling independent from graph and page scrolling. */
    #scroll = (event) => {
        if (!(event.target instanceof Node) || !this.#contains(event.target))
            this.#dismiss();
    };
    /** Let the feature decide how dismissal changes its state. */
    #dismiss = () => this.#options?.dismiss();
    /** Prevent host graph controls from receiving popup input. */
    #stop = (event) => event.stopPropagation();
    /** Prevent a browser-native menu from replacing an owned menu. */
    #context = (event) => {
        event.preventDefault();
        event.stopPropagation();
    };
    /** Include sibling submenu portals and the originating action in outside-click ownership. */
    #contains(target) {
        return this.element.contains(target) || (this.#options?.containsTarget?.(target) ?? false);
    }
}
