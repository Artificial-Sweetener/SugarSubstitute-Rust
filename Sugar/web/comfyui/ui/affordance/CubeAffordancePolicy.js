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
/** Decide Cube affordances without importing host, browser, or persistence concerns. */
import { isDraftCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
/** Own the product distinction between structural Subgraphs and SugarCubes. */
export class CubeAffordancePolicy {
    /** Decide one action from shared selection and editor context. */
    decide(actionId, selection, editor) {
        const targetKind = resolveTargetKind(selection, editor);
        const native = nativeDecision(actionId, targetKind);
        if (actionId === 'convert' || actionId === 'unpack') {
            return selection.containsCube
                ? decision(actionId, targetKind, 'blocked', actionId === 'convert'
                    ? 'SugarCube cannot be converted'
                    : 'SugarCube cannot be unpacked', false, false)
                : native;
        }
        if (actionId === 'publish' && selection.isSingleCube) {
            return decision(actionId, targetKind, 'save-cube', 'Save Cube', true, true);
        }
        if (actionId === 'configure-interface' && (selection.containsCube || editor?.isCubeRoot)) {
            return decision(actionId, targetKind, 'blocked', 'Edit Subgraph Widgets', false, false);
        }
        if (actionId === 'node-info' && selection.isSingleCube) {
            return decision(actionId, targetKind, 'blocked', 'Cube details', false, false);
        }
        if (actionId === 'rename-instance' && selection.isSingleCube) {
            return decision(actionId, targetKind, 'rename-cube-instance', 'Rename Cube instance', true, true);
        }
        if (!editor?.isCubeRoot)
            return native;
        if (actionId === 'save-workflow') {
            return decision(actionId, targetKind, 'save-cube-editor', 'Save Cube', true, true);
        }
        if (actionId === 'exit-container') {
            return decision(actionId, targetKind, 'exit-cube', 'Exit Cube', true, true);
        }
        if (actionId === 'set-description') {
            return decision(actionId, targetKind, 'edit-cube-metadata', 'Edit Cube metadata', true, true);
        }
        if (actionId === 'set-search-aliases') {
            return decision(actionId, targetKind, 'blocked', 'Set Search Aliases', false, false);
        }
        if (actionId === 'clear-container') {
            return decision(actionId, targetKind, 'clear-cube', 'Clear Cube implementation', true, true);
        }
        return native;
    }
}
/** Classify the current action target before applying action-specific rules. */
function resolveTargetKind(selection, editor) {
    if (editor)
        return editor.isCubeRoot ? 'cube-editor-root' : 'nested-subgraph';
    if (selection.isSingleCube) {
        const node = selection.cubeNodes[0];
        return node && isDraftCubeNode(node) ? 'draft-cube' : 'saved-cube';
    }
    if (selection.containsCube)
        return 'mixed-with-cube';
    if (selection.isSingleOrdinarySubgraph)
        return 'ordinary-subgraph';
    return 'none';
}
/** Preserve Comfy behavior when no Cube-domain rule applies. */
function nativeDecision(actionId, targetKind) {
    const labels = {
        convert: 'Convert to Subgraph',
        unpack: 'Unpack Subgraph',
        publish: 'Publish Subgraph',
        'configure-interface': 'Edit Subgraph Widgets',
        'node-info': 'Node Info',
        'rename-instance': 'Rename',
        'save-workflow': 'Save Workflow',
        'exit-container': 'Exit Subgraph',
        'set-description': 'Set Subgraph Description',
        'set-search-aliases': 'Set Subgraph Search Aliases',
        'clear-container': 'Clear Workflow',
    };
    return decision(actionId, targetKind, 'native', labels[actionId], true, true);
}
/** Construct a complete immutable decision without implicit defaults. */
function decision(actionId, targetKind, intent, label, visible, enabled) {
    return { actionId, enabled, intent, label, targetKind, visible };
}
