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
/** Translate Photoshop's layer hierarchy and primary selection into portable source identities. */
import { action } from 'photoshop';
import type { CanvasSourceLayer } from '../../../contracts/CanvasProtocol.generated.js';
import { canvasLayersAboveTarget } from '../../../shared/CanvasSourceLayers.js';
import type { PhotoshopDocument } from './PhotoshopCanvasIdentity.js';

type NativeLayer = PhotoshopDocument['layers'][number];

/** Retain native handles only inside the editor boundary that owns their lifetime. */
export interface PhotoshopSourceLayerCatalog {
  readonly values: readonly CanvasSourceLayer[];
  readonly handles: ReadonlyMap<string, NativeLayer>;
}

/** Enumerate the document in visual stack order, preserving group ancestry and stable layer IDs. */
export function photoshopSourceLayers(document: PhotoshopDocument): PhotoshopSourceLayerCatalog {
  const values: CanvasSourceLayer[] = [],
    handles = new Map<string, NativeLayer>();
  /** Traverse actual native children without flattening groups into a competing layer model. */
  function visit(layers: readonly NativeLayer[], parentId: string | null, depth: number): void {
    if (depth > 128) throw new Error('The Photoshop layer hierarchy exceeds its supported depth.');
    for (const layer of layers) {
      if (values.length >= 4096)
        throw new Error('The Photoshop layer catalog exceeds its supported size.');
      const id = String(layer.id);
      if (handles.has(id)) throw new Error('Photoshop returned a duplicate layer identity.');
      handles.set(id, layer);
      values.push({ id, name: layer.name.slice(0, 256), parentId, depth, visible: layer.visible });
      const children = layer.layers;
      if (children) visit([...children], id, depth + 1);
    }
  }
  visit([...document.layers], null, 0);
  return { values, handles };
}

/** Resolve the primary selected layer even when Photoshop has several layers selected. */
export async function photoshopCurrentLayer(document: PhotoshopDocument): Promise<string | null> {
  const active = document.activeLayers;
  if (!active.length) return null;
  if (active.length === 1) return String(active[0]?.id);
  const response: unknown = await action.batchPlay(
    [{ _obj: 'get', _target: [{ _ref: 'layer', _enum: 'ordinal', _value: 'targetEnum' }] }],
    {},
  );
  const value: unknown = Array.isArray(response) ? response[0] : null;
  if (
    !value ||
    typeof value !== 'object' ||
    !('layerID' in value) ||
    typeof value.layerID !== 'number' ||
    !Number.isSafeInteger(value.layerID) ||
    !active.some((layer) => layer.id === value.layerID)
  )
    throw new Error('Photoshop did not identify its primary selected layer.');
  return String(value.layerID);
}

/** Read a composition range while restoring each visibility flag before another target is captured. */
export async function withPhotoshopCompositionRange<T>(
  catalog: PhotoshopSourceLayerCatalog,
  layerId: string,
  capture: () => Promise<T>,
): Promise<T> {
  const hidden: NativeLayer[] = [];
  try {
    for (const id of canvasLayersAboveTarget(catalog.values, layerId)) {
      const layer = catalog.handles.get(id);
      if (!layer) throw new Error('The Photoshop layer catalog changed during capture.');
      if (!layer.visible) continue;
      hidden.push(layer);
      layer.visible = false;
    }
    return await capture();
  } finally {
    for (const layer of hidden) layer.visible = true;
  }
}
