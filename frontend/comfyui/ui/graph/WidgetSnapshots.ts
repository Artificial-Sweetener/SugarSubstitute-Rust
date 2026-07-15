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
/**
 * Capture request-only, name-addressed Comfy widget values.
 */

import { getGraphNodes } from './GraphQuery.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyGraph, ComfyNode, ComfyWidget } from '../types/graph.js';

export interface WorkflowWithNodes extends UnknownRecord {
  nodes?: ComfyNode[];
}

/** Request-only workflow field carrying widget values keyed by stable name. */
export const WORKFLOW_WIDGET_VALUES_KEY = 'sugarcubes_widget_values';

function isSerializedWidget(widget: ComfyWidget | null | undefined): boolean {
  if (!widget || typeof widget.name !== 'string' || !widget.name.trim()) {
    return false;
  }
  if (widget.serialize === false || widget.options?.serialize === false) {
    return false;
  }
  return widget.type !== 'button';
}

function cloneJsonValue(value: unknown): unknown | undefined {
  if (value === undefined || typeof value === 'function' || typeof value === 'symbol') {
    return undefined;
  }
  try {
    const cloned: unknown = JSON.parse(JSON.stringify(value));
    return cloned;
  } catch (_error) {
    return undefined;
  }
}

function serializedWidgetNames(node: ComfyNode): string[] {
  const names: string[] = [];
  for (const input of Array.isArray(node?.inputs) ? node.inputs : []) {
    const name =
      typeof input?.widget?.name === 'string' && input.widget.name.trim()
        ? input.widget.name.trim()
        : '';
    if (!name) {
      continue;
    }
    if (names.includes(name)) {
      throw new Error(`Serialized node '${node?.id ?? ''}' has duplicate widget name '${name}'.`);
    }
    names.push(name);
  }
  return names;
}

function decodeSerializedWidgetValues(
  node: ComfyNode,
  liveWidgets: readonly ComfyWidget[],
): Map<string, unknown> {
  const names = serializedWidgetNames(node);
  const persisted = Array.isArray(node?.widgets_values) ? node.widgets_values : [];
  const values = new Map<string, unknown>();
  let valueIndex = 0;
  let companionValuesRemaining = Math.max(0, persisted.length - names.length);

  for (const name of names) {
    if (valueIndex >= persisted.length) {
      throw new Error(`Serialized node '${node?.id ?? ''}' is missing widget value '${name}'.`);
    }
    values.set(name, persisted[valueIndex]);
    valueIndex += 1;

    const liveIndex = liveWidgets.findIndex((widget) => widget?.name === name);
    const companion = liveIndex >= 0 ? liveWidgets[liveIndex + 1] : null;
    if (
      companion &&
      !isSerializedWidget(companion) &&
      companionValuesRemaining > 0 &&
      valueIndex < persisted.length
    ) {
      valueIndex += 1;
      companionValuesRemaining -= 1;
    }
  }

  if (valueIndex !== persisted.length) {
    throw new Error(
      `Serialized node '${node?.id ?? ''}' has positional widget values without stable names.`,
    );
  }
  return values;
}

/**
 * Return one live node's serializable widget values keyed by widget name.
 */
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
    const value = cloneJsonValue(widget.value ?? widget.last_value ?? widget.options?.value);
    if (value !== undefined) {
      values[name] = value;
    }
  }
  return values;
}

/**
 * Attach request-only widget snapshots to matching workflow nodes.
 */
export function attachWorkflowWidgetSnapshots(
  workflow: WorkflowWithNodes,
  graph: ComfyGraph | null | undefined,
): WorkflowWithNodes {
  if (!Array.isArray(workflow.nodes)) {
    return workflow;
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
  return workflow;
}

/**
 * Rebuild subgraph widget arrays from persisted names and current host defaults.
 */
export function rebindSubgraphWidgetValues(
  subgraph: WorkflowWithNodes,
  createNode: ((type: string | undefined) => ComfyNode | null) | null | undefined,
): WorkflowWithNodes {
  if (!Array.isArray(subgraph.nodes)) {
    return subgraph;
  }
  if (typeof createNode !== 'function') {
    throw new Error('Current node factory is unavailable for widget rebinding.');
  }

  for (const node of subgraph.nodes) {
    if (!Array.isArray(node?.widgets_values)) {
      continue;
    }
    const liveNode = createNode(node.type || node.class_type);
    if (!liveNode) {
      throw new Error(`Node type '${node.type || node.class_type}' is unavailable.`);
    }
    const liveWidgets = Array.isArray(liveNode.widgets) ? liveNode.widgets : [];
    const persistedByName = decodeSerializedWidgetValues(node, liveWidgets);
    node.widgets_values = liveWidgets.map((widget) => {
      const persisted = persistedByName.get(widget?.name);
      if (isSerializedWidget(widget) && persisted !== undefined && persisted !== null) {
        return cloneJsonValue(persisted);
      }
      return cloneJsonValue(widget?.value ?? widget?.last_value ?? widget?.options?.value) ?? null;
    });
  }
  return subgraph;
}
