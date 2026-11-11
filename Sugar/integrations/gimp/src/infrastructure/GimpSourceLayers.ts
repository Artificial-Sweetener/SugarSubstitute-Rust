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
/** Translate portable layer targeting into visibility overrides on an owned native snapshot. */
import type { CanvasSourceTarget } from '../../../contracts/CanvasProtocol.generated.js';
import {
  canvasLayerIsolation,
  canvasLayersAboveTarget,
} from '../../../shared/CanvasSourceLayers.js';
import type { NativeLayer, NativeVisibility } from '../contracts/NativeProtocol.generated.js';
import { GimpNativeFailure, gimpCanvasLayers } from './GimpCanvasMetadata.js';

/** Resolve the frozen primary layer and shared hierarchy policy before touching native visibility. */
export function gimpSourceVisibility(
  target: CanvasSourceTarget,
  layers: readonly NativeLayer[],
  primary: number | null,
): readonly NativeVisibility[] {
  if (target.kind === 'selection' || target.kind === 'composition') return [];
  const id =
    target.kind === 'current-layer' ? (primary === null ? null : String(primary)) : target.layerId;
  if (!id || !layers.some((layer) => String(layer.tattoo) === id))
    throw new GimpNativeFailure('stale', 'The selected GIMP source layer is unavailable.');
  const catalog = gimpCanvasLayers(layers);
  const visibility =
    target.kind === 'composition-from-layer'
      ? canvasLayersAboveTarget(catalog, id).map((id) => ({ id, visible: false }))
      : canvasLayerIsolation(catalog, id);
  const tattoos = new Map(layers.map((layer) => [String(layer.tattoo), layer.tattoo]));
  return visibility.map((change) => {
    const tattoo = tattoos.get(change.id);
    if (tattoo === undefined) throw new Error('The frozen GIMP layer catalog is inconsistent.');
    return { tattoo, visible: change.visible };
  });
}
