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
/**
 * Keep the graph's grid out from under node cards in the desktop frame. A card there is a
 * translucent fill over the window material, so anything the graph draws beneath it shows through
 * it. The grid is erased where a card stands, leaving the plain body wash a masonry card stands on;
 * the card's fill and the material behind it are untouched. ComfyUI's node opacity setting is the
 * strength of that erasure: 1 hides the grid behind every card, lower values let it through in
 * proportion. ComfyUI itself fades the card by the same setting, which is left as it is.
 */
import { isRecord } from '../../types/common.js';
import { addGraphBackgroundPainter } from '../../overlays/GraphBackgroundPainters.js';
/** ComfyUI's node opacity setting, 0 to 1. */
export const NODE_OPACITY_SETTING = 'Comfy.Node.Opacity';
/** A node card's corner radius in graph units; the card's own CSS rounds it the same. */
export const NODE_CARD_RADIUS = 4;
/** How much of the grid to erase under cards for a stored setting value; unset means all of it. */
export function occlusionStrength(setting) {
    return typeof setting === 'number' && Number.isFinite(setting)
        ? Math.min(1, Math.max(0, setting))
        : 1;
}
/** Erase the background under every node whose bounds reach into the visible area. */
export function eraseUnderNodes(context, nodes, visible, strength) {
    if (strength <= 0)
        return;
    context.save();
    context.globalCompositeOperation = 'destination-out';
    context.fillStyle = `rgba(0,0,0,${String(strength)})`;
    context.beginPath();
    for (const node of nodes) {
        const bounds = isRecord(node) ? readBounds(node.boundingRect) : null;
        if (!bounds || (visible && !overlaps(bounds, visible)))
            continue;
        context.roundRect(bounds[0], bounds[1], bounds[2], bounds[3], NODE_CARD_RADIUS);
    }
    context.fill();
    context.restore();
}
/** Own the erasure for one graph canvas while the desktop frame is mounted. */
export class DesktopGraphNodeOcclusion {
    #release = null;
    /** Paint on `canvas` and follow the setting through `app`; attaching again replaces the first. */
    attach(app, canvas) {
        this.detach();
        const settings = isRecord(app) && isRecord(app.ui) ? app.ui.settings : null;
        const redraw = () => {
            if (typeof canvas.setDirty === 'function')
                canvas.setDirty(true, true);
        };
        const removePainter = addGraphBackgroundPainter(canvas, (context) => {
            const graph = isRecord(canvas.graph) ? canvas.graph : null;
            const nodes = graph && Array.isArray(graph._nodes) ? graph._nodes : [];
            eraseUnderNodes(context, nodes, readBounds(canvas.visible_area), occlusionStrength(readSetting(app)));
        });
        const event = `${NODE_OPACITY_SETTING}.change`;
        const followed = isRecord(settings) &&
            typeof settings.addEventListener === 'function' &&
            typeof settings.removeEventListener === 'function'
            ? settings
            : null;
        if (followed)
            followed.addEventListener.call(followed, event, redraw);
        this.#release = () => {
            removePainter();
            if (followed)
                followed.removeEventListener.call(followed, event, redraw);
            redraw();
        };
        redraw();
    }
    /** Stop painting and let the grid return under the cards. */
    detach() {
        this.#release?.();
        this.#release = null;
    }
}
function readSetting(app) {
    const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
    const setting = manager && isRecord(manager.setting) ? manager.setting : null;
    return setting && typeof setting.get === 'function'
        ? setting.get.call(setting, NODE_OPACITY_SETTING)
        : undefined;
}
function readBounds(value) {
    if (!value || typeof value !== 'object' || !('length' in value))
        return null;
    const bounds = value;
    const [x, y, width, height] = [bounds[0], bounds[1], bounds[2], bounds[3]];
    return x !== undefined &&
        y !== undefined &&
        width !== undefined &&
        height !== undefined &&
        [x, y, width, height].every(Number.isFinite) &&
        width > 0 &&
        height > 0
        ? [x, y, width, height]
        : null;
}
function overlaps(a, b) {
    const [ax, ay, aw, ah, bx, by, bw, bh] = [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]].map((value) => value ?? 0);
    return ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by;
}
