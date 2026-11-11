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
/** Adapt graph-owned model widgets to selection receivers while their node cards are absent. */
import { isRecord } from '../types/common.js';
import { findModelGraphNode, locateModelGraph } from './ComfyModelGraph.js';
/** Resolve native graph identity once, then validate it against the current workflow on every use. */
export function comfyModelFieldOwner(app, root, binding) {
    const located = locateModelGraph(app, binding);
    if (!located)
        return { identity: root, exists: () => root.isConnected, receiver: () => null };
    const node = located.node;
    const name = binding.inputName ?? binding.label;
    /** Read the refreshed native widget rather than retaining stale choices after installation. */
    const widget = () => Array.isArray(node.widgets)
        ? (node.widgets.find((value) => isRecord(value) && value.name === name) ?? null)
        : null;
    return {
        identity: node,
        exists: () => findModelGraphNode(app, node) !== null,
        receiver: () => {
            const scope = findModelGraphNode(app, node);
            const current = widget();
            if (!scope || !isRecord(current) || typeof current.value !== 'string')
                return null;
            const options = isRecord(current.options) ? current.options : {};
            const values = Array.isArray(options.values)
                ? options.values.filter((value) => typeof value === 'string')
                : [];
            return {
                options: values,
                readValue: () => (typeof current.value === 'string' ? current.value : ''),
                disabled: () => current.disabled === true || options.disabled === true,
                writeValue: (value) => {
                    const previous = current.value;
                    current.value = value;
                    if (typeof current.callback === 'function')
                        current.callback.call(current, value);
                    if (typeof node.onWidgetChanged === 'function')
                        node.onWidgetChanged.call(node, name, value, previous, current);
                    if (typeof scope.graph.afterChange === 'function')
                        scope.graph.afterChange.call(scope.graph);
                },
            };
        },
    };
}
