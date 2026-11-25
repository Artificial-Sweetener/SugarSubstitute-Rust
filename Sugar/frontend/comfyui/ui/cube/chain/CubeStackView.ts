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
 * The stack as the page sends it to SugarCubes' backend: the Cube order and the proximity links
 * the page's matcher shows. The backend uses both as given, so a queued run, its saved images and
 * a saved project read the stack exactly as the page shows it.
 */
import { isRecord } from '../../types/common.js';
import { requireCubeIdentity, type CubeNode } from '../node/ComfyCubeNodeFactory.js';

/** One proximity link between two Cube boundaries, by instance id and boundary name. */
export interface ProximityConnectionRecord {
  readonly source_instance_id: string;
  readonly source_binding: string;
  readonly target_instance_id: string;
  readonly target_binding: string;
}

/** The stack the page sends with a workflow. */
export interface CubeStackView {
  readonly order: readonly string[];
  readonly proximity_connections: readonly ProximityConnectionRecord[];
}

/** Describe the stack from its order and the proximity matches the page shows. */
export function cubeStackView(
  order: readonly string[],
  matches: readonly unknown[],
): CubeStackView {
  return { order: [...order], proximity_connections: matches.flatMap(proximityConnection) };
}

/** Project one Cube-to-Cube proximity match onto stable boundary identities. */
function proximityConnection(value: unknown): ProximityConnectionRecord[] {
  if (!isRecord(value)) return [];
  const match = value;
  const stable = readStableMatchConnection(match);
  if (stable) return [stable];
  const outputCube = isCubeLike(match.outputNode) ? match.outputNode : match.outputCube;
  const inputCube = isCubeLike(match.inputNode) ? match.inputNode : match.inputCube;
  if (!isCubeLike(outputCube) || !isCubeLike(inputCube)) return [];
  const sourceInstanceId = readString(requireCubeIdentity(outputCube).instance_id);
  const targetInstanceId = readString(requireCubeIdentity(inputCube).instance_id);
  const sourceBinding = readBoundaryName(outputCube.subgraph.outputs, match.outputSlot);
  const targetBinding = readBoundaryName(inputCube.subgraph.inputs, match.inputSlot);
  if (!sourceInstanceId || !targetInstanceId || !sourceBinding || !targetBinding) return [];
  return [
    {
      source_instance_id: sourceInstanceId,
      source_binding: sourceBinding,
      target_instance_id: targetInstanceId,
      target_binding: targetBinding,
    },
  ];
}

/** Project matcher-owned endpoint identity without inspecting host node shape. */
function readStableMatchConnection(
  match: Record<string, unknown>,
): ProximityConnectionRecord | null {
  if (match.outputCube == null || match.inputCube == null) return null;
  const sourceInstanceId = readString(match.outputInstanceId);
  const sourceBinding = readString(match.outputBinding);
  const targetInstanceId = readString(match.inputInstanceId);
  const targetBinding = readString(match.inputBinding);
  if (!sourceInstanceId || !sourceBinding || !targetInstanceId || !targetBinding) return null;
  return {
    source_instance_id: sourceInstanceId,
    source_binding: sourceBinding,
    target_instance_id: targetInstanceId,
    target_binding: targetBinding,
  };
}

/** Narrow a native Cube surface without coupling to its renderer. */
function isCubeLike(value: unknown): value is CubeNode {
  return (
    isRecord(value) &&
    isRecord(value.properties) &&
    isRecord(value.subgraph) &&
    Array.isArray(value.subgraph.inputs) &&
    Array.isArray(value.subgraph.outputs)
  );
}

/** Read one ordered native boundary name. */
function readBoundaryName(entries: unknown[], slotValue: unknown): string {
  if (typeof slotValue !== 'number' || !Number.isInteger(slotValue)) return '';
  if (slotValue < 0 || slotValue >= entries.length) return '';
  const entry = entries[slotValue];
  return isRecord(entry) ? readString(entry.name) : '';
}

/** Read one non-empty external string. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
