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
/** Keep scroll tracks in their native containing surface and translate viewport geometry locally. */
import { scrollParent, scrollbarHorizontalOutset } from './FluentScrollbarLayout.js';
/** Share layout-neutral layers within a surface without modifying host styles or reserving gutters. */
export class FluentScrollbarLayers {
    document;
    #layers = new Map();
    #owners = new Map();
    /** Bind layer creation to the native document while retaining shadow-tree ownership. */
    constructor(document) {
        this.document = document;
    }
    /** Retain the same thumbs through reparenting and let ancestor clipping and stacking apply naturally. */
    mount(owner, vertical, horizontal) {
        const surface = this.#surface(owner);
        if (this.#owners.get(owner) !== surface)
            this.release(owner);
        let layer = this.#layers.get(surface);
        if (!layer) {
            const element = this.document.createElement('div');
            element.className = 'sc-fluent-scrollbar-layer';
            surface.append(element);
            layer = { element, owners: new Map(), horizontalOutset: 0 };
            this.#layers.set(surface, layer);
        }
        const outset = scrollbarHorizontalOutset(owner);
        if (layer.owners.get(owner) !== outset) {
            layer.owners.set(owner, outset);
            this.#measureOutset(layer);
        }
        this.#outset(layer);
        this.#owners.set(owner, surface);
        if (vertical.parentElement !== layer.element)
            layer.element.append(vertical, horizontal);
        return layer.element;
    }
    /** Release an empty layer as soon as its last owner disappears or moves elsewhere. */
    release(owner) {
        const surface = this.#owners.get(owner);
        if (!surface)
            return;
        this.#owners.delete(owner);
        const layer = this.#layers.get(surface);
        layer?.owners.delete(owner);
        if (layer && layer.owners.size === 0) {
            layer.element.remove();
            this.#layers.delete(surface);
        }
        else if (layer) {
            this.#measureOutset(layer);
            this.#outset(layer);
        }
    }
    /** Remove only owned layers when Fluent appearance is disabled. */
    clear() {
        for (const layer of this.#layers.values())
            layer.element.remove();
        this.#layers.clear();
        this.#owners.clear();
    }
    /** Extend paint into an opted-in surface's existing breathing room without reserving layout space. */
    #outset(layer) {
        const pixels = layer.horizontalOutset;
        if (!pixels) {
            layer.element.style.removeProperty('bottom');
            return;
        }
        const bottom = `${-pixels / scrollbarCoordinateSpace(layer.element).scaleY}px`;
        if (layer.element.style.bottom !== bottom)
            layer.element.style.bottom = bottom;
    }
    /** Recompute the shared allowance only when owner membership or configuration changes. */
    #measureOutset(layer) {
        let maximum = 0;
        for (const outset of layer.owners.values())
            maximum = Math.max(maximum, outset);
        layer.horizontalOutset = maximum;
    }
    /** Keep handles inside the closest native positioning, paint, or stacking boundary. */
    #surface(owner) {
        const window = this.document.defaultView;
        for (let element = owner; element; element = scrollParent(element)) {
            const style = window?.getComputedStyle(element);
            const parentDisplay = style?.zIndex && style.zIndex !== 'auto' && element.parentElement
                ? window?.getComputedStyle(element.parentElement).display
                : '';
            if (element !== this.document.documentElement &&
                element !== this.document.body &&
                style &&
                ((style.position && style.position !== 'static') ||
                    /^(inline-)?(flex|grid)$/.test(parentDisplay ?? '') ||
                    style.isolation === 'isolate' ||
                    (style.transform && style.transform !== 'none') ||
                    (style.filter && style.filter !== 'none') ||
                    /layout|paint|strict|content/.test(style.contain)))
                return element;
            const root = element.getRootNode();
            if (!element.parentElement && root instanceof ShadowRoot)
                return root;
        }
        return this.document.body;
    }
}
/** Convert through the rendered layer's origin and scale, including native graph zoom and scrolling. */
export function scrollbarCoordinateSpace(layer) {
    const rect = layer.getBoundingClientRect();
    return {
        left: rect.left,
        top: rect.top,
        scaleX: layer.offsetWidth && rect.width ? rect.width / layer.offsetWidth : 1,
        scaleY: layer.offsetHeight && rect.height ? rect.height / layer.offsetHeight : 1,
    };
}
