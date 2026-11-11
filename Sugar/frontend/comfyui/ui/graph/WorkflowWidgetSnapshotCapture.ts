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
/** Capture request-only, name-addressed Comfy widget values. */

import { getGraphNodes } from './GraphQuery.js';
import { cloneWidgetValue, isSerializedWidget } from './WidgetValueSerialization.js';
import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph, ComfyNode } from '../types/graph.js';

export interface WorkflowWithNodes extends UnknownRecord {
  nodes?: ComfyNode[];
  definitions?: UnknownRecord;
}

/** Request-only workflow field carrying widget values keyed by stable name. */
export const WORKFLOW_WIDGET_VALUES_KEY = 'sugarcubes_widget_values';

/** Return one live node's serializable widget values keyed by widget name. */
export function captureNodeWidgetValues(node: ComfyNode | null | undefined): UnknownRecord {
  const values: UnknownRecord = {};
  for (const widget of Array.isArray(node?.widgets) ? node.widgets : []) {
    if (!isSerializedWidget(widget)) {
      continue;
    }
    const name = widget.name.trim();
    if (Object.prototype.hasOwnProperty.call(values, name)) {
      throw new Error(`Node '${node?.id ?? ''}' has duplicate widget name '${name}'.`);
    }
    const value = cloneWidgetValue(widget.value ?? widget.last_value ?? widget.options?.value);
    if (value !== undefined) {
      values[name] = value;
    }
  }
  return values;
}

/** Attach request-only widget snapshots to matching workflow nodes. */
export function attachWorkflowWidgetSnapshots(
  workflow: WorkflowWithNodes,
  graph: ComfyGraph | null | undefined,
): WorkflowWithNodes {
  attachNodeWidgetSnapshots(workflow, graph);
  const definitions = isRecord(workflow.definitions) ? workflow.definitions : {};
  const subgraphs = Array.isArray(definitions.subgraphs) ? definitions.subgraphs : [];
  const liveSubgraphs = graph?._subgraphs instanceof Map ? graph._subgraphs : null;
  if (!liveSubgraphs) {
    return workflow;
  }
  for (const subgraph of subgraphs) {
    if (
      !isRecord(subgraph) ||
      (typeof subgraph.id !== 'string' && typeof subgraph.id !== 'number')
    ) {
      continue;
    }
    const liveSubgraph = findLiveSubgraph(liveSubgraphs, subgraph.id);
    if (liveSubgraph) {
      attachNodeWidgetSnapshots(subgraph, liveSubgraph);
    }
  }
  return workflow;
}

/** Attach live widget values to one serialized graph's matching nodes. */
function attachNodeWidgetSnapshots(
  workflow: WorkflowWithNodes,
  graph: ComfyGraph | null | undefined,
): void {
  if (!Array.isArray(workflow.nodes)) {
    return;
  }
  const nodesById = new Map(
    getGraphNodes(graph)
      .filter((node) => node?.id != null)
      .map((node) => [String(node.id), node]),
  );
  for (const workflowNode of workflow.nodes) {
    if (!workflowNode || workflowNode.id == null) {
      continue;
    }
    const liveNode = nodesById.get(String(workflowNode.id));
    if (!liveNode) {
      continue;
    }
    const values = captureNodeWidgetValues(liveNode);
    if (Object.keys(values).length) {
      workflowNode[WORKFLOW_WIDGET_VALUES_KEY] = values;
    }
  }
}

/** Resolve a live subgraph without assuming the host map's key representation. */
function findLiveSubgraph(
  subgraphs: Map<string | number, unknown>,
  id: string | number,
): ComfyGraph | null {
  const direct = subgraphs.get(id);
  if (isRecord(direct)) {
    return direct;
  }
  const expected = String(id);
  for (const [candidateId, candidate] of subgraphs) {
    if (String(candidateId) === expected && isRecord(candidate)) {
      return candidate;
    }
  }
  return null;
}
