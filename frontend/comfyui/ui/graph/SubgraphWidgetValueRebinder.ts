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
/** Rebind persisted subgraph widget values to current fields by stable name. */

import {
  cloneWidgetValue,
  isSerializedWidget,
  readCurrentWidgetValue,
} from './WidgetValueSerialization.js';
import { indexSubgraphBoundaryWidgetNames } from './SubgraphBoundaryWidgetIdentities.js';
import type { UnknownRecord } from '../types/common.js';
import type { ComfyLink, ComfyNode, ComfyWidget } from '../types/graph.js';

export interface SubgraphWithNodes extends UnknownRecord {
  links?: ComfyLink[];
  nodes?: ComfyNode[];
}

/** Rebuild subgraph widget arrays against the complete current widget layout. */
export function rebindSubgraphWidgetValues(
  subgraph: SubgraphWithNodes,
  createNode: ((type: string | undefined) => ComfyNode | null) | null | undefined,
): SubgraphWithNodes {
  if (!Array.isArray(subgraph.nodes)) {
    return subgraph;
  }
  if (typeof createNode !== 'function') {
    throw new Error('Current node factory is unavailable for widget rebinding.');
  }

  const boundaryNamesByNode = indexSubgraphBoundaryWidgetNames(subgraph);

  for (const node of subgraph.nodes) {
    if (!Array.isArray(node?.widgets_values)) {
      continue;
    }
    const liveNode = createNode(node.type || node.class_type);
    if (!liveNode) {
      throw new Error(`Node type '${node.type || node.class_type}' is unavailable.`);
    }
    const liveWidgets = Array.isArray(liveNode.widgets) ? liveNode.widgets : [];
    const boundaryNames = boundaryNamesByNode.get(String(node.id ?? '')) ?? new Set<string>();
    const persistedByName = decodeSerializedWidgetValues(node, liveWidgets, boundaryNames);
    node.widgets_values = rebuildWidgetValues(liveWidgets, persistedByName);
  }
  return subgraph;
}

/** Decode a canonical saved array using the field identities stored beside it. */
function decodeSerializedWidgetValues(
  node: ComfyNode,
  liveWidgets: readonly ComfyWidget[],
  includedLinkedNames: ReadonlySet<string>,
): Map<string, unknown> {
  const names = serializedWidgetNames(node, includedLinkedNames);
  const persisted = Array.isArray(node?.widgets_values) ? node.widgets_values : [];
  if (!names.length && persisted.length && linkedWidgetNames(node).size) {
    return new Map();
  }
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

/** Return unlinked widget identities from the same snapshot as the saved values. */
function serializedWidgetNames(
  node: ComfyNode,
  includedLinkedNames: ReadonlySet<string>,
): string[] {
  const names: string[] = [];
  for (const input of Array.isArray(node?.inputs) ? node.inputs : []) {
    const name =
      typeof input?.widget?.name === 'string' && input.widget.name.trim()
        ? input.widget.name.trim()
        : '';
    if (input?.link != null && !includedLinkedNames.has(name)) {
      continue;
    }
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

/** Build the complete current array while applying portable values only by name. */
function rebuildWidgetValues(
  liveWidgets: readonly ComfyWidget[],
  persistedByName: ReadonlyMap<string, unknown>,
): unknown[] {
  return liveWidgets.map((widget) => {
    if (!widget) {
      return null;
    }
    const persisted = persistedByName.get(widget.name);
    return isSerializedWidget(widget) && persisted !== undefined && persisted !== null
      ? cloneWidgetValue(persisted)
      : readCurrentWidgetValue(widget);
  });
}

/** Return widget identities whose authoritative values arrive through graph links. */
function linkedWidgetNames(node: ComfyNode): Set<string> {
  return new Set(
    (Array.isArray(node.inputs) ? node.inputs : [])
      .filter((input) => input?.link != null)
      .map((input) => input.widget?.name?.trim() ?? '')
      .filter(Boolean),
  );
}
