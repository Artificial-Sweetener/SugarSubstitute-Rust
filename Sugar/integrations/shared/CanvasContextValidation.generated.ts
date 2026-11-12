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
/** Generated protocol validation; edit the authoritative canvas.schema.json. */
import type * as Contract from '../contracts/CanvasProtocol.generated.js';
import { isCanvasDocument } from './CanvasDocumentValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasLayerCapabilities payload before dispatch. */
export function isCanvasLayerCapabilities(
  value: unknown,
): value is Contract.CanvasLayerCapabilities {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'displayOutput') &&
    typeof value['displayOutput'] === 'boolean' &&
    Object.hasOwn(value, 'insertImageLayer') &&
    typeof value['insertImageLayer'] === 'boolean' &&
    Object.hasOwn(value, 'positionImageLayer') &&
    typeof value['positionImageLayer'] === 'boolean'
  );
}
/** Validate a bounded CanvasGeometryCapabilities payload before dispatch. */
export function isCanvasGeometryCapabilities(
  value: unknown,
): value is Contract.CanvasGeometryCapabilities {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'displayOutput') &&
    typeof value['displayOutput'] === 'boolean' &&
    Object.hasOwn(value, 'insertImageLayer') &&
    typeof value['insertImageLayer'] === 'boolean' &&
    Object.hasOwn(value, 'positionImageLayer') &&
    typeof value['positionImageLayer'] === 'boolean' &&
    Object.hasOwn(value, 'geometry') &&
    typeof value['geometry'] === 'boolean'
  );
}
/** Validate a bounded CanvasEditingCapabilities payload before dispatch. */
export function isCanvasEditingCapabilities(
  value: unknown,
): value is Contract.CanvasEditingCapabilities {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'displayOutput') &&
    typeof value['displayOutput'] === 'boolean' &&
    Object.hasOwn(value, 'insertImageLayer') &&
    typeof value['insertImageLayer'] === 'boolean' &&
    Object.hasOwn(value, 'positionImageLayer') &&
    typeof value['positionImageLayer'] === 'boolean' &&
    Object.hasOwn(value, 'geometry') &&
    typeof value['geometry'] === 'boolean' &&
    Object.hasOwn(value, 'editing') &&
    typeof value['editing'] === 'boolean' &&
    Object.hasOwn(value, 'workflowSurfaces') &&
    typeof value['workflowSurfaces'] === 'boolean'
  );
}
/** Validate a bounded CanvasCapabilities payload before dispatch. */
export function isCanvasCapabilities(value: unknown): value is Contract.CanvasCapabilities {
  return (
    isCanvasLayerCapabilities(value) ||
    isCanvasGeometryCapabilities(value) ||
    isCanvasEditingCapabilities(value)
  );
}
/** Validate a bounded CanvasDescriptor payload before dispatch. */
export function isCanvasDescriptor(value: unknown): value is Contract.CanvasDescriptor {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 128 &&
    Object.hasOwn(value, 'label') &&
    typeof value['label'] === 'string' &&
    value['label'].length <= 128 &&
    Object.hasOwn(value, 'capabilities') &&
    isCanvasCapabilities(value['capabilities'])
  );
}
/** Validate a bounded CanvasContext payload before dispatch. */
export function isCanvasContext(value: unknown): value is Contract.CanvasContext {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'canvas') &&
    isCanvasDescriptor(value['canvas']) &&
    Object.hasOwn(value, 'document') &&
    (isCanvasDocument(value['document']) || value['document'] === null)
  );
}
