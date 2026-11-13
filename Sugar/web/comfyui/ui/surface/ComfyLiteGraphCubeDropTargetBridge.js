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
/** Route Nodes 1 file drops through the native internal node under the cursor. */
import { containsCubeCanvasPoint } from './CubeCanvasLayout.js';
/** Preserve Comfy's root-node drop contract while resolving one internal card generically. */
export class ComfyLiteGraphCubeDropTargetBridge {
    #items = new Map();
    #mounted = new Map();
    /** Reconcile native callback wrappers with the currently visible Cube faces. */
    sync(items) {
        const active = new Set(items.map((item) => item.node));
        for (const node of this.#mounted.keys()) {
            if (!active.has(node))
                this.#unmount(node);
        }
        this.#items.clear();
        for (const item of items)
            this.#items.set(item.node, item);
        for (const item of items) {
            const hooks = this.#mounted.get(item.node);
            if (hooks && ownsInstalledCallbacks(item.node, hooks))
                continue;
            if (hooks)
                this.#mounted.delete(item.node);
            this.#mount(item.node);
        }
    }
    /** Restore every Cube callback exactly as Comfy supplied it. */
    dispose() {
        for (const node of [...this.#mounted.keys()])
            this.#unmount(node);
        this.#items.clear();
    }
    /** Install one root-node façade without changing any internal callback. */
    #mount(node) {
        const originalDragOver = node.onDragOver;
        const originalDragDrop = node.onDragDrop;
        const installedDragOver = (event) => {
            const target = this.#resolveTarget(node, event);
            if (target)
                return target.onDragOver?.call(target, event) ?? false;
            return originalDragOver?.call(node, event) ?? false;
        };
        const installedDragDrop = (event) => {
            const target = this.#resolveTarget(node, event);
            if (target)
                return target.onDragDrop?.call(target, event) ?? false;
            return originalDragDrop?.call(node, event) ?? false;
        };
        const hooks = {
            originalDragOver,
            originalDragDrop,
            ownedDragOver: Object.prototype.hasOwnProperty.call(node, 'onDragOver'),
            ownedDragDrop: Object.prototype.hasOwnProperty.call(node, 'onDragDrop'),
            installedDragOver,
            installedDragDrop,
        };
        node.onDragOver = installedDragOver;
        node.onDragDrop = installedDragDrop;
        this.#mounted.set(node, hooks);
    }
    /** Restore one Cube's precise own-property callback state. */
    #unmount(node) {
        const hooks = this.#mounted.get(node);
        if (!hooks)
            return;
        if (node.onDragOver === hooks.installedDragOver) {
            restoreCallback(node, 'onDragOver', hooks.originalDragOver, hooks.ownedDragOver);
        }
        if (node.onDragDrop === hooks.installedDragDrop) {
            restoreCallback(node, 'onDragDrop', hooks.originalDragDrop, hooks.ownedDragDrop);
        }
        this.#mounted.delete(node);
    }
    /** Resolve Comfy-adjusted graph coordinates to the visible internal card. */
    #resolveTarget(node, event) {
        const adjusted = event;
        const point = [Number(adjusted.canvasX), Number(adjusted.canvasY)];
        if (!point.every(Number.isFinite))
            return null;
        const item = this.#items.get(node);
        return item?.cards.find((card) => containsCubeCanvasPoint(card.rect, point))?.node ?? null;
    }
}
/** Return whether Comfy still exposes the callbacks installed by this bridge. */
function ownsInstalledCallbacks(node, hooks) {
    return node.onDragOver === hooks.installedDragOver && node.onDragDrop === hooks.installedDragDrop;
}
/** Restore an optional callback without manufacturing an inherited own property. */
function restoreCallback(node, key, value, owned) {
    Reflect.set(node, key, value);
    if (!owned)
        Reflect.deleteProperty(node, key);
}
