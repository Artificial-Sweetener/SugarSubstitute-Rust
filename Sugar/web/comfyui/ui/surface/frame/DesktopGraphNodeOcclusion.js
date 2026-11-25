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
 * Let node cards hide what lies beneath them in the desktop frame, as Comfy's opaque cards do. A
 * card there is a translucent fill over the window material, so anything beneath it would show
 * through it. Everything the graph canvas draws beneath the cards (the grid, groups, links,
 * reroutes and the proximity noodles) is erased where a card stands, and a card that lies under
 * another card is masked where that card covers it (`DesktopNodeStacking`). Only the window
 * material shows through a card. ComfyUI's node opacity setting is the strength of both: 1 hides
 * everything beneath a card, lower values let it through in proportion, as in Comfy, which also
 * fades the card itself by the same setting.
 */
import { isRecord } from '../../types/common.js';
import { addGraphForegroundPainter } from '../../overlays/GraphForegroundPainters.js';
import { applyStacking, clearStacking } from './DesktopNodeStacking.js';
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
/** Erase everything drawn beneath every node whose bounds reach into the visible area. */
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
        let stacked = [];
        const removePainter = addGraphForegroundPainter(canvas, (context) => {
            const graph = isRecord(canvas.graph) ? canvas.graph : null;
            const nodes = graph && Array.isArray(graph._nodes) ? graph._nodes : [];
            const strength = occlusionStrength(readSetting(app));
            eraseUnderNodes(context, nodes, readBounds(canvas.visible_area), strength);
            const cards = stackedCards(canvas, nodes);
            stacked = cards.map((card) => card.element);
            applyStacking(cards, strength);
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
            clearStacking(stacked);
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
/** The graph's node cards as Comfy stacks them: by z-index, then by their order in the graph. */
function stackedCards(canvas, nodes) {
    const element = canvas.canvas;
    const document = element instanceof HTMLElement ? element.ownerDocument : null;
    if (!document)
        return [];
    const byId = new Map();
    for (const card of document.querySelectorAll('.lg-node[data-node-id]'))
        if (!card.closest('.sugarcubes-native-node-card'))
            byId.set(card.dataset.nodeId ?? '', card);
    const cards = [];
    nodes.forEach((node, index) => {
        if (!isRecord(node))
            return;
        const card = byId.get(String(node.id));
        const bounds = readBounds(node.boundingRect);
        if (!card || !bounds)
            return;
        const z = Number.parseInt(card.style.zIndex, 10);
        cards.push({ element: card, bounds, order: (Number.isFinite(z) ? z : 0) * 1e6 + index });
    });
    return cards;
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
