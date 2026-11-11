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
/** Own projected-card display text without changing native Vue models or custom controls. */
import { formatDisplayLabel } from './DisplayLabels.js';
/** Follow native text replacement and restore only text still owned by the theme. */
export class NativeLabelPresentation {
    #cards = new Map();
    /** Format projected-card labels and restore native text when a card returns to the graph. */
    reconcile(root, elements) {
        for (const [card, labels] of this.#cards) {
            if (!card.isConnected) {
                this.#restore(labels);
                this.#cards.delete(card);
            }
        }
        if (!root.closest('.sugarcubes-cube-face__node-card')) {
            const labels = this.#cards.get(root);
            if (labels)
                this.#restore(labels);
            this.#cards.delete(root);
            return;
        }
        const labels = this.#cards.get(root) ?? new Map();
        this.#cards.set(root, labels);
        const current = new Set();
        for (const element of elements) {
            if (!root.contains(element) || element.childElementCount > 0)
                continue;
            for (const node of element.childNodes)
                if (node instanceof Text)
                    current.add(node);
        }
        for (const [node, label] of labels) {
            if (current.has(node))
                continue;
            restoreLabel(node, label);
            labels.delete(node);
        }
        for (const node of current) {
            const previous = labels.get(node);
            const source = previous && node.data === previous.displayed ? previous.source : node.data;
            const displayed = formatDisplayLabel(source);
            labels.set(node, { source, displayed });
            if (node.data !== displayed)
                node.data = displayed;
        }
    }
    /** Return display ownership to Comfy without overwriting a newer native edit. */
    clear() {
        for (const labels of this.#cards.values())
            this.#restore(labels);
        this.#cards.clear();
    }
    /** Restore retained text nodes even when their native card has been detached. */
    #restore(labels) {
        for (const [node, label] of labels)
            restoreLabel(node, label);
    }
}
/** Avoid replacing host updates that arrived before the next reconciliation. */
function restoreLabel(node, label) {
    if (node.data === label.displayed)
        node.data = label.source;
}
