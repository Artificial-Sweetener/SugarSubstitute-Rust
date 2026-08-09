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
/** Transfer live instance state between versions through stable node and field identities. */

import { applyInputValueToNode } from '../../import/ImportNodeWriter.js';
import { captureNodeWidgetValues } from '../../graph/WorkflowWidgetSnapshotCapture.js';
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import {
  parseCubeSurfaceState,
  serializeCubeSurfaceState,
  type CubeSurfaceState,
} from '../../surface/CubeSurfaceState.js';
import { requireCubeSurface, type CubeNode } from '../node/ComfyCubeNodeFactory.js';
import type { NativeGraphNode } from '../ComfyCubeGraphBuilder.js';

/** Apply source live values and presentation state only where stable identities still match. */
export function transferCubeVersionState(source: CubeNode, target: CubeNode): void {
  const sourceBySymbol = indexNodesBySymbol(source);
  const targetBySymbol = indexNodesBySymbol(target);
  for (const [symbol, sourceNode] of sourceBySymbol) {
    const targetNode = targetBySymbol.get(symbol);
    if (!targetNode) continue;
    transferNodeFields(sourceNode, targetNode);
    if (typeof sourceNode.mode === 'number') targetNode.mode = sourceNode.mode;
  }
  const surface = transferSurfaceState(source, target, sourceBySymbol, targetBySymbol);
  replaceRecord(requireCubeSurface(target), serializeCubeSurfaceState(surface));
}

/** Transfer name-addressed serialized widget values without overwriting target-only defaults. */
function transferNodeFields(source: NativeGraphNode, target: NativeGraphNode): void {
  for (const [name, value] of Object.entries(captureNodeWidgetValues(source))) {
    applyInputValueToNode(target, name, value);
  }
}

/** Re-key matching card state from old runtime IDs onto the replacement definition. */
function transferSurfaceState(
  source: CubeNode,
  target: CubeNode,
  sourceBySymbol: ReadonlyMap<string, NativeGraphNode>,
  targetBySymbol: ReadonlyMap<string, NativeGraphNode>,
): CubeSurfaceState {
  const sourceState = parseCubeSurfaceState(requireCubeSurface(source));
  const targetState = parseCubeSurfaceState(requireCubeSurface(target));
  const sourceSymbolById = new Map(
    [...sourceBySymbol].map(([symbol, node]) => [String(node.id), symbol]),
  );
  const targetIdBySymbol = new Map(
    [...targetBySymbol].map(([symbol, node]) => [symbol, String(node.id)]),
  );
  const cards = { ...targetState.cards };
  for (const [sourceId, card] of Object.entries(sourceState.cards)) {
    const targetId = targetIdBySymbol.get(sourceSymbolById.get(sourceId) ?? '');
    if (targetId) cards[targetId] = { ...card };
  }
  const transferredOrder = sourceState.nodeOrder.flatMap((sourceId) => {
    const targetId = targetIdBySymbol.get(sourceSymbolById.get(sourceId) ?? '');
    return targetId ? [targetId] : [];
  });
  const nodeOrder = [...transferredOrder];
  for (const targetId of targetState.nodeOrder) {
    if (!nodeOrder.includes(targetId)) nodeOrder.push(targetId);
  }
  const outputNames = new Set(target.outputs.map((output) => output.name).filter(Boolean));
  return {
    ...targetState,
    nodeOrder,
    cards,
    minimumColumnWidth: sourceState.minimumColumnWidth,
    gap: sourceState.gap,
    preview: {
      ...sourceState.preview,
      selectedOutput:
        sourceState.preview.selectedOutput && outputNames.has(sourceState.preview.selectedOutput)
          ? sourceState.preview.selectedOutput
          : null,
    },
  };
}

/** Index internal nodes by their durable Cube symbol. */
function indexNodesBySymbol(node: CubeNode): Map<string, NativeGraphNode> {
  const indexed = new Map<string, NativeGraphNode>();
  for (const internal of node.subgraph._nodes) {
    const symbol = isRecord(internal.properties)
      ? readString(internal.properties.sugarcubes_symbol)
      : '';
    if (!symbol) continue;
    if (indexed.has(symbol)) throw new Error(`Cube node symbol '${symbol}' is duplicated.`);
    indexed.set(symbol, internal);
  }
  return indexed;
}

/** Replace one graph-owned record without changing its object identity. */
function replaceRecord(target: UnknownRecord, source: object): void {
  for (const key of Object.keys(target)) Reflect.deleteProperty(target, key);
  Object.assign(target, source);
}

/** Read a trimmed stable identifier. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
