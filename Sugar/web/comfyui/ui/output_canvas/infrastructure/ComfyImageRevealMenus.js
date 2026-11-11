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
/** Add one qualified inspection command while retaining native node image-menu ownership. */
import { isRecord } from '../../types/common.js';
/** Borrow and restore only cached native source nodes' public menu callback. */
export class ComfyImageRevealMenus {
    target;
    reveal;
    #bindings = new Map();
    #contexts = new Map();
    /** Bind current thumbnail resolution and the shared inspection command. */
    constructor(target, reveal) {
        this.target = target;
        this.reveal = reveal;
    }
    /** Capture DOM gallery selection for the next native context-menu construction. */
    context(path, target) {
        this.#contexts.set(path, target);
    }
    /** Reconcile cached source membership without scanning graph topology or replacing existing menu entries. */
    reconcile(sources) {
        const active = new Set(sources.map((s) => s.node));
        for (const [node, binding] of this.#bindings)
            if (!active.has(node)) {
                if (node.getExtraMenuOptions === binding.wrapped)
                    node.getExtraMenuOptions = binding.original;
                this.#bindings.delete(node);
                this.#contexts.delete(binding.path);
            }
        for (const { path, node } of sources) {
            if (this.#bindings.has(node))
                continue;
            const original = node.getExtraMenuOptions;
            const owner = this;
            const wrapped = function (...args) {
                const result = typeof original === 'function' ? Reflect.apply(original, this, args) : undefined;
                const options = args[1];
                const target = owner.#contexts.get(path) ?? owner.#selected(path, node);
                owner.#contexts.delete(path);
                if (target && Array.isArray(options))
                    options.push({ content: 'Show in output canvas', callback: () => owner.reveal(target) });
                return result;
            };
            node.getExtraMenuOptions = wrapped;
            this.#bindings.set(node, { original, wrapped, path });
        }
    }
    /** Restore every borrowed callback on renderer, workflow, or page teardown. */
    dispose() {
        this.reconcile([]);
        this.#contexts.clear();
    }
    /** Respect Nodes 1's selected or hovered image without inventing a batch index for transient media. */
    #selected(path, node) {
        const index = typeof node.imageIndex === 'number' ? node.imageIndex : node.overIndex;
        const images = Array.isArray(node.imgs) ? node.imgs : [];
        const image = typeof index === 'number' ? images[index] : images.length === 1 ? images[0] : null;
        return isRecord(image) && typeof image.src === 'string'
            ? this.target(path, image.src, typeof index === 'number' ? index : images.length === 1 ? 0 : null)
            : null;
    }
}
