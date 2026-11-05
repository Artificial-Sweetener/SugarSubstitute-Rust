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
/** Place a floating contextual toolbar beside its context with stable side hysteresis. */
/** Gap between the context and the toolbar. */
export const CANVAS_EDIT_TOOLBAR_GAP = 12;
/** Viewport inset kept free of floating chrome. */
export const CANVAS_EDIT_CHROME_INSET = 8;
/** Bottom overflow, in physical pixels, needed to flip sides; the margin prevents flicker. */
const EDGE_HYSTERESIS_PHYSICAL_PX = 24;
/**
 * Retain the chosen side across context changes, or the place the user dragged the toolbar to.
 *
 * The toolbar sits centred below its context, moves above once the context's bottom overflows
 * the safe area by the hysteresis margin, and returns below only after the context clears it by
 * the same margin. A context spanning the whole safe height, or no context at all, places the
 * toolbar centred along the bottom. Once dragged, the toolbar keeps its centre at the same
 * fraction of the safe area while the context, the viewport and its own width change, until the
 * context is reset. Every result is clamped completely inside the safe area.
 */
export class CanvasToolbarPlacement {
    #side = null;
    /** The dragged centre as fractions of the safe area, or null for automatic placement. */
    #dragged = null;
    /** Forget the retained side and any dragged place so the next context is placed afresh. */
    reset() {
        this.#side = null;
        this.#dragged = null;
    }
    /**
     * Move the toolbar from where it is shown by a pointer delta and keep it there.
     *
     * @param from The toolbar's shown top-left point, in the safe area's coordinate space.
     * @param delta The pointer movement in the same CSS pixels.
     * @returns The clamped top-left point to show.
     */
    moveBy(from, delta, size, safe) {
        const moved = clamp({ x: from.x + delta.x, y: from.y + delta.y }, size, safe);
        this.#dragged = {
            x: (moved.x + size.width / 2 - safe.left) / Math.max(1, safe.width),
            y: (moved.y + size.height / 2 - safe.top) / Math.max(1, safe.height),
        };
        return moved;
    }
    /** Project the toolbar's top-left point for one context, size and safe area. */
    position(context, size, safe, physicalPixelsPerCssPixel) {
        const dragged = this.#dragged;
        if (dragged)
            return clamp({
                x: safe.left + dragged.x * Math.max(1, safe.width) - size.width / 2,
                y: safe.top + dragged.y * Math.max(1, safe.height) - size.height / 2,
            }, size, safe);
        const side = context ? this.#resolve(context, safe, physicalPixelsPerCssPixel) : null;
        this.#side = side;
        const bottom = safe.top + safe.height;
        if (!context || side === 'viewport-bottom')
            return clamp({ x: safe.left + safe.width / 2 - size.width / 2, y: bottom - size.height }, size, safe);
        const x = context.left + context.width / 2 - size.width / 2;
        const y = side === 'above'
            ? context.top - CANVAS_EDIT_TOOLBAR_GAP - size.height
            : context.top + context.height + CANVAS_EDIT_TOOLBAR_GAP;
        return clamp({ x, y }, size, safe);
    }
    #resolve(context, safe, density) {
        const safeBottom = safe.top + safe.height;
        const contextBottom = context.top + context.height;
        if (context.top < safe.top && contextBottom > safeBottom)
            return 'viewport-bottom';
        const overflow = (contextBottom - safeBottom) * Math.max(1, density);
        if (this.#side === 'above')
            return overflow <= -EDGE_HYSTERESIS_PHYSICAL_PX ? 'below' : 'above';
        return overflow > EDGE_HYSTERESIS_PHYSICAL_PX ? 'above' : 'below';
    }
}
/** Inset a viewport rectangle by the chrome margin. */
export function canvasEditSafeArea(viewport) {
    return {
        left: viewport.left + CANVAS_EDIT_CHROME_INSET,
        top: viewport.top + CANVAS_EDIT_CHROME_INSET,
        width: Math.max(0, viewport.width - 2 * CANVAS_EDIT_CHROME_INSET),
        height: Math.max(0, viewport.height - 2 * CANVAS_EDIT_CHROME_INSET),
    };
}
/** Keep a rectangle of the given size inside the safe area, preferring its top-left edge. */
function clamp(point, size, safe) {
    const maxX = Math.max(safe.left, safe.left + safe.width - size.width);
    const maxY = Math.max(safe.top, safe.top + safe.height - size.height);
    return {
        x: Math.round(Math.min(Math.max(point.x, safe.left), maxX)),
        y: Math.round(Math.min(Math.max(point.y, safe.top), maxY)),
    };
}
