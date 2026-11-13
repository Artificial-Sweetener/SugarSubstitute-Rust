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
/** Restore instance-specific promoted values after Comfy creates durable widget identities. */
import { isRecord } from '../types/common.js';
import { getGraphNodes } from '../graph/GraphQuery.js';
import { cloneWidgetValue, isSerializedWidget } from '../graph/WidgetValueSerialization.js';
/** Keep Cube-owned nested instances separate from shared subgraph definition defaults. */
export class CubeNestedWidgetRestoration {
    #snapshots = [];
    /** Discard unfinished hydration before preparing the next workflow. */
    reset() {
        this.#snapshots.length = 0;
    }
    /** Capture current-schema values only for nested nodes inside a Cube-owned definition. */
    capture(definition, definitionIds, createNode) {
        for (const node of Array.isArray(definition.nodes) ? definition.nodes : []) {
            if (!isRecord(node) || typeof node.type !== 'string' || !definitionIds.has(node.type))
                continue;
            if (!Array.isArray(node.widgets_values))
                continue;
            const values = new Map();
            const names = (Array.isArray(node.inputs) ? node.inputs : []).flatMap((input) => isRecord(input) && isRecord(input.widget) && typeof input.widget.name === 'string'
                ? [input.widget.name]
                : []);
            if (names.length === node.widgets_values.length && new Set(names).size === names.length) {
                for (const [index, name] of names.entries())
                    values.set(name, cloneWidgetValue(node.widgets_values[index]));
            }
            else {
                const widgets = createNode(node.type)?.widgets;
                if (!widgets || widgets.length !== node.widgets_values.length)
                    continue;
                for (const [index, widget] of widgets.entries()) {
                    if (isSerializedWidget(widget))
                        values.set(widget.name, cloneWidgetValue(node.widgets_values[index]));
                }
            }
            this.#snapshots.push({
                graphId: String(definition.id),
                nodeId: String(node.id),
                values,
            });
        }
    }
    /** Apply saved instance values through native setters after host schema migration and identity setup. */
    restore(graph) {
        const definitions = graph?.subgraphs ?? graph?._subgraphs;
        if (!(definitions instanceof Map)) {
            this.reset();
            return;
        }
        const graphs = new Map();
        for (const [id, candidate] of definitions) {
            if (isRecord(candidate))
                graphs.set(String(id), candidate);
        }
        for (const snapshot of this.#snapshots) {
            const node = getGraphNodes(graphs.get(snapshot.graphId)).find((candidate) => String(candidate.id) === snapshot.nodeId);
            for (const widget of node?.widgets ?? []) {
                if (snapshot.values.has(widget.name))
                    widget.value = cloneWidgetValue(snapshot.values.get(widget.name));
            }
        }
        this.reset();
    }
}
