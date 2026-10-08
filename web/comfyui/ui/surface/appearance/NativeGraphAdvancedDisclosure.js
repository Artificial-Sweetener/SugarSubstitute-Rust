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
/** Adapt native advanced-field disclosure to the shared Fluent accordion motion. */
import { CardDisclosureMotion } from '../motion/CardDisclosureMotion.js';
import { NATIVE_NODE_CARD_DISPOSE_EVENT } from '../NativeNodeCardRenderer.js';
import { settleNativeDisclosure } from '../NativeDisclosureSettlement.js';
import { measureNativeFields } from './NativeAdvancedDisclosureGeometry.js';
/** Preserve native advanced state and callbacks while owning only disclosure presentation. */
export class NativeGraphAdvancedDisclosure {
    #bindings = new Map();
    /** Follow native component replacement and the current advanced state. */
    reconcile(card) {
        for (const [root, binding] of this.#bindings) {
            if (!root.isConnected || (root === card.root && binding.fields !== card.advanced?.fields)) {
                binding.release();
                this.#bindings.delete(root);
            }
        }
        if (!card.advanced)
            return;
        const existing = this.#bindings.get(card.root);
        if (existing) {
            existing.button = card.advanced.button;
            existing.expanded = card.advanced.expanded;
            return;
        }
        const { fields, button, expanded } = card.advanced;
        let forwarding = false;
        let dispatching = false;
        const dispose = () => {
            binding.release();
            this.#bindings.delete(card.root);
        };
        const binding = {
            fields,
            button,
            expanded,
            motion: new CardDisclosureMotion(fields),
            release: () => {
                card.root.removeEventListener('click', click, true);
                card.root.removeEventListener(NATIVE_NODE_CARD_DISPOSE_EVENT, dispose);
                binding.motion.dispose();
            },
        };
        const click = (event) => {
            if (forwarding ||
                !(event.target instanceof Element) ||
                !binding.button.contains(event.target))
                return;
            event.stopImmediatePropagation();
            dispatching = true;
            try {
                binding.motion.toggle(!binding.expanded, (collapsed) => {
                    if (binding.expanded === !collapsed)
                        return;
                    // Native HTMLElement.click suppresses reentrant clicks on the same button.
                    const invoke = () => {
                        if (binding.expanded === !collapsed)
                            return;
                        forwarding = true;
                        try {
                            binding.button.click();
                        }
                        finally {
                            forwarding = false;
                        }
                        binding.expanded = !collapsed;
                    };
                    if (dispatching)
                        queueMicrotask(invoke);
                    else
                        invoke();
                }, {
                    closedHeight: () => measureNativeFields(fields, true),
                    expandedHeight: () => measureNativeFields(fields, false),
                    settle: () => settleNativeDisclosure(fields),
                });
            }
            finally {
                dispatching = false;
            }
        };
        card.root.addEventListener('click', click, true);
        card.root.addEventListener(NATIVE_NODE_CARD_DISPOSE_EVENT, dispose);
        this.#bindings.set(card.root, binding);
    }
    /** Commit pending native choices and release transient motion when appearance ends. */
    clear() {
        for (const binding of this.#bindings.values())
            binding.release();
        this.#bindings.clear();
    }
}
