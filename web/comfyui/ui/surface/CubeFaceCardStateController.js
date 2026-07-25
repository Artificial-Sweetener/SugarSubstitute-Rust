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
/** Own reveal and activation transitions for optional Cube-face cards. */
import { resolveCubeFaceRevealDecision } from './CubeFaceRevealPolicy.js';
const BYPASS_MODE = 4;
/** Persist one reveal override while retaining its independent activation choice. */
export function setCubeFaceCardRevealed(state, node, revealed) {
    const nodeId = String(node.id ?? '');
    const current = state.cards[nodeId];
    const decision = resolveCubeFaceRevealDecision(node, current);
    if (!decision.revealable)
        return;
    const activeMode = resolveActiveMode(node, current?.activeMode);
    state.cards[nodeId] = {
        authoredBypass: true,
        revealed,
        enabledOverride: decision.enabledChoice,
        activeMode,
    };
    node.mode = revealed && decision.enabledChoice ? activeMode : BYPASS_MODE;
}
/** Change execution while retaining the policy-owned reveal state. */
export function setCubeFaceNodeEnabled(state, node, enabled) {
    const nodeId = String(node.id ?? '');
    const current = state.cards[nodeId];
    const decision = resolveCubeFaceRevealDecision(node, current);
    const activeMode = resolveActiveMode(node, current?.activeMode);
    state.cards[nodeId] = {
        authoredBypass: decision.authoredBypass,
        revealed: decision.revealed,
        enabledOverride: enabled,
        activeMode,
    };
    node.mode = enabled && decision.visible ? activeMode : BYPASS_MODE;
}
/** Retain the last executable mode while a reveal or activation policy bypasses the node. */
function resolveActiveMode(node, persistedMode) {
    const currentMode = finiteNodeMode(node.mode);
    return currentMode !== BYPASS_MODE ? currentMode : (persistedMode ?? 0);
}
/** Narrow dynamic LiteGraph modes to finite integer state. */
function finiteNodeMode(value) {
    return typeof value === 'number' && Number.isInteger(value) && value >= 0 ? value : 0;
}
