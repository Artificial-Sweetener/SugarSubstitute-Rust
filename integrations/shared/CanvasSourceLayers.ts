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
/** Resolve source ranges against an explicit top-to-bottom hierarchical canvas layer catalog. */
import type { CanvasSourceLayer } from '../contracts/CanvasProtocol.generated.js';

/** Exclude layers above a target while retaining its ancestors and every layer below it. */
export function canvasLayersAboveTarget(
  layers: readonly CanvasSourceLayer[],
  targetId: string,
): readonly string[] {
  const byId = new Map(layers.map((layer) => [layer.id, layer]));
  if (byId.size !== layers.length)
    throw new Error('The canvas layer catalog contains duplicate identities.');
  const target = byId.get(targetId);
  if (!target) throw new Error('The selected canvas layer is unavailable.');
  const ancestors = new Set<string>();
  let parentId = target.parentId;
  while (parentId !== null) {
    if (ancestors.has(parentId) || parentId === targetId)
      throw new Error('The canvas layer hierarchy contains a cycle.');
    const parent = byId.get(parentId);
    if (!parent) throw new Error('The canvas layer hierarchy is incomplete.');
    ancestors.add(parentId);
    parentId = parent.parentId;
  }
  const hidden = new Set<string>();
  const omitted = new Set<string>();
  for (const layer of layers) {
    if (layer.id === targetId) break;
    if (ancestors.has(layer.id)) continue;
    if (layer.parentId !== null && omitted.has(layer.parentId)) {
      omitted.add(layer.id);
      continue;
    }
    hidden.add(layer.id);
    omitted.add(layer.id);
  }
  return [...hidden];
}
