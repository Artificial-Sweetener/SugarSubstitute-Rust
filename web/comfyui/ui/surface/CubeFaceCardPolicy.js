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
/** Resolve renderer-neutral Cube-face card visibility and activation controls. */
import { cubeFaceNodeHasVisibleWidgets } from './CubeFaceNodePresentationPolicy.js';
import { cubeFaceNodeAllowsActivationControl, inferCubeFaceTransformSignals, isHardHiddenCubeFaceNode, } from './CubeFaceNodeSemantics.js';
import { resolveCubeFaceRevealDecision } from './CubeFaceRevealPolicy.js';
/** Resolve the exact cards and menu entries consumed by either Comfy renderer. */
export function resolveCubeFaceCardPresentation(nodes, state) {
    const cards = [];
    const menuEntries = [];
    nodes.forEach((node, index) => {
        if (isHardHiddenCubeFaceNode(node))
            return;
        const id = String(node.id ?? index);
        const persisted = state.cards[id];
        const reveal = resolveCubeFaceRevealDecision(node, persisted);
        const showActivationControl = cubeFaceNodeAllowsActivationControl(node) &&
            (reveal.revealable || inferCubeFaceTransformSignals(node).size > 0);
        const eligible = cubeFaceNodeHasVisibleWidgets(node) || showActivationControl || reveal.revealable;
        if (!eligible)
            return;
        const label = node.title?.trim() || node.type?.trim() || `Node ${String(index + 1)}`;
        if (reveal.revealable) {
            menuEntries.push({ id, label, revealed: reveal.revealed });
        }
        cards.push({
            id,
            node,
            label,
            visible: reveal.visible,
            enabled: reveal.enabled,
            showActivationControl,
        });
    });
    return { cards, menuEntries };
}
