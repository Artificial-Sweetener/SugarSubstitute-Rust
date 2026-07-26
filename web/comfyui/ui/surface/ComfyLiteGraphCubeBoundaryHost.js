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
/** Position Nodes 1.0 native output slots across from Cube preview titles. */
import { resolveCubeCanvasInputSocketCenterX, resolveCubeCanvasOutputSocketCenterX, } from './CubeCanvasBoundaryGeometry.js';
/** Own reversible presentation changes to real LiteGraph output slots. */
export class ComfyLiteGraphCubeBoundaryHost {
    #presentations = new Map();
    #slotHeight;
    /** Bind Comfy's native slot geometry without owning its visual styling. */
    constructor(options = {}) {
        this.#slotHeight = readPositiveNumber(options.slotHeight, readPositiveNumber(globalThis.LiteGraph?.NODE_SLOT_HEIGHT, 20));
    }
    /** Synchronize native hit targets with current transient boundary geometry. */
    sync(node, inputs, outputs) {
        const presentations = this.#presentations.get(node) ?? new Map();
        this.#presentations.set(node, presentations);
        const current = new Set();
        for (const input of inputs) {
            const slot = asLiteGraphSlot(node.inputs[input.index]);
            if (!slot)
                continue;
            current.add(slot);
            rememberSlot(presentations, slot);
            slot.pos = [
                resolveCubeCanvasInputSocketCenterX(0, this.#slotHeight),
                input.y - Number(node.pos[1]),
            ];
        }
        for (const output of outputs) {
            const slot = asLiteGraphSlot(node.outputs[output.index]);
            if (!slot)
                continue;
            current.add(slot);
            rememberSlot(presentations, slot);
            slot.pos = [
                resolveCubeCanvasOutputSocketCenterX(Number(node.size[0]), this.#slotHeight),
                output.y - Number(node.pos[1]),
            ];
        }
        for (const slot of presentations.keys()) {
            if (!current.has(slot)) {
                restoreSlot(slot, presentations.get(slot));
                presentations.delete(slot);
            }
        }
    }
    /** Draw only native dots while Cube-owned gutters render readable labels. */
    drawNativeSlotDots(node, context, drawSlots) {
        context.save();
        context.beginPath();
        for (const value of [...node.inputs, ...node.outputs]) {
            const slot = asLiteGraphSlot(value);
            const position = asPosition(slot?.pos);
            if (!position)
                continue;
            context.moveTo(position[0] + 8, position[1]);
            context.arc(position[0], position[1], 8, 0, Math.PI * 2);
        }
        context.clip();
        try {
            drawSlots();
        }
        finally {
            context.restore();
        }
    }
    /** Restore every output slot owned by one released Cube node. */
    release(node) {
        const presentations = this.#presentations.get(node);
        if (!presentations)
            return;
        for (const [slot, presentation] of presentations)
            restoreSlot(slot, presentation);
        this.#presentations.delete(node);
    }
    /** Restore all mounted Cube output slots. */
    dispose() {
        for (const node of [...this.#presentations.keys()])
            this.release(node);
    }
}
/** Retain one slot's exact optional presentation before moving it. */
function rememberSlot(presentations, slot) {
    if (presentations.has(slot))
        return;
    presentations.set(slot, {
        hadPosition: Object.hasOwn(slot, 'pos'),
        position: slot.pos,
    });
}
/** Narrow one dynamic host slot to the presentation surface LiteGraph reads. */
function asLiteGraphSlot(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value)
        ? value
        : null;
}
/** Restore the exact optional properties present before Cube presentation. */
function restoreSlot(slot, presentation) {
    if (!presentation)
        return;
    restoreOptional(slot, 'pos', presentation.hadPosition, presentation.position);
}
/** Narrow an optional host slot position to finite local canvas coordinates. */
function asPosition(value) {
    if (!Array.isArray(value) && !ArrayBuffer.isView(value))
        return null;
    const x = Number(Reflect.get(value, '0'));
    const y = Number(Reflect.get(value, '1'));
    return Number.isFinite(x) && Number.isFinite(y) ? [x, y] : null;
}
/** Read a finite positive host geometry value without propagating invalid state. */
function readPositiveNumber(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) && number > 0 ? number : fallback;
}
/** Restore one optionally owned host property without inventing undefined state. */
function restoreOptional(slot, key, existed, value) {
    if (existed) {
        slot[key] = value;
    }
    else {
        Reflect.deleteProperty(slot, key);
    }
}
