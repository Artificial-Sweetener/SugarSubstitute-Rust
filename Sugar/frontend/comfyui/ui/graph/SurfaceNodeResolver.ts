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
 * Resolve live graph nodes that own SugarCubes surface controls.
 */

import { getGraphNodes } from './GraphQuery.js';
import type {
  ComfyGraph,
  ComfyNode,
  CubeSurface,
  GraphId,
  SurfaceControl,
} from '../types/graph.js';

/**
 * Return a live node id as the string form used by instance metadata.
 */
function readNodeId(node: ComfyNode | null | undefined): string {
  return node?.id != null ? String(node.id) : '';
}

/**
 * Return the persisted SugarCubes symbol assigned to a live node.
 */
function readNodeSymbol(node: ComfyNode | null | undefined): string {
  return typeof node?.properties?.sugarcubes_symbol === 'string'
    ? node.properties.sugarcubes_symbol.trim()
    : '';
}

/**
 * Return the node class type used by surface control metadata.
 */
function readNodeClassType(node: ComfyNode | null | undefined): string {
  if (typeof node?.type === 'string' && node.type.trim()) {
    return node.type.trim();
  }
  if (typeof node?.class_type === 'string' && node.class_type.trim()) {
    return node.class_type.trim();
  }
  return '';
}

/**
 * Read a normalized string field from a surface control.
 */
function readControlString(control: SurfaceControl, key: string): string {
  const value = control?.[key];
  return typeof value === 'string' ? value.trim() : '';
}

/**
 * Group surface controls by their declared owner symbol.
 */
function groupControlsBySymbol(
  surface: CubeSurface | null | undefined,
): Map<string, SurfaceControl[]> {
  const groups = new Map<string, SurfaceControl[]>();
  const controls = Array.isArray(surface?.controls) ? surface.controls : [];
  for (const control of controls) {
    const symbol = readControlString(control, 'symbol');
    if (!symbol) {
      continue;
    }
    const entries = groups.get(symbol) || [];
    entries.push(control);
    groups.set(symbol, entries);
  }
  return groups;
}

/**
 * Return whether a live node exposes a widget-backed or property-backed input.
 */
function nodeHasInputName(node: ComfyNode | null | undefined, inputName: string): boolean {
  if (!node || !inputName) {
    return false;
  }
  if (Array.isArray(node.widgets) && node.widgets.some((widget) => widget?.name === inputName)) {
    return true;
  }
  if (
    Array.isArray(node.inputs) &&
    node.inputs.some((input) => input?.name === inputName || input?.widget?.name === inputName)
  ) {
    return true;
  }
  return Boolean(
    node.properties && Object.prototype.hasOwnProperty.call(node.properties, inputName),
  );
}

/**
 * Return whether a node can satisfy the declared control class type.
 */
function nodeMatchesControlClass(node: ComfyNode, controls: readonly SurfaceControl[]): boolean {
  const classTypes = new Set(
    controls.map((control) => readControlString(control, 'class_type')).filter(Boolean),
  );
  if (!classTypes.size) {
    return true;
  }
  return classTypes.has(readNodeClassType(node));
}

/**
 * Return whether a node exposes all inputs for one surface-control owner.
 */
function nodeOwnsSurfaceControls(node: ComfyNode, controls: readonly SurfaceControl[]): boolean {
  const inputNames = controls
    .map((control) => readControlString(control, 'input_name'))
    .filter(Boolean);
  return (
    inputNames.length > 0 && inputNames.every((inputName) => nodeHasInputName(node, inputName))
  );
}

/**
 * Infer a missing symbol only when the managed instance has one clear owner.
 */
function inferMissingSymbolNode(
  nodes: readonly ComfyNode[],
  symbol: string,
  controls: readonly SurfaceControl[],
): ComfyNode | null {
  const candidates = nodes.filter((node) => {
    const nodeSymbol = readNodeSymbol(node);
    return (
      (!nodeSymbol || nodeSymbol === symbol) &&
      nodeMatchesControlClass(node, controls) &&
      nodeOwnsSurfaceControls(node, controls)
    );
  });
  return candidates.length === 1 ? (candidates[0] ?? null) : null;
}

/**
 * Build a live-node lookup for surface controls.
 */
export function buildSurfaceNodesBySymbol(
  graph: ComfyGraph | null | undefined,
  nodeIds: readonly GraphId[] | null | undefined,
  surface: CubeSurface | null | undefined,
): Map<string, ComfyNode> {
  const nodeIdSet = new Set((Array.isArray(nodeIds) ? nodeIds : []).map((value) => String(value)));
  const nodes: ComfyNode[] = getGraphNodes(graph).filter((node: ComfyNode) => {
    const nodeId = readNodeId(node);
    return nodeId && nodeIdSet.has(nodeId);
  });
  const nodesBySymbol = new Map<string, ComfyNode>();
  for (const node of nodes) {
    const symbol = readNodeSymbol(node);
    if (symbol && !nodesBySymbol.has(symbol)) {
      nodesBySymbol.set(symbol, node);
    }
  }
  for (const [symbol, controls] of groupControlsBySymbol(surface)) {
    if (nodesBySymbol.has(symbol)) {
      continue;
    }
    const inferred = inferMissingSymbolNode(nodes, symbol, controls);
    if (inferred) {
      nodesBySymbol.set(symbol, inferred);
    }
  }
  return nodesBySymbol;
}
