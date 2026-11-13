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
import { isCanvasSurfaceFileReference } from './CanvasSurfaceValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasWorkflowSurfaceBase payload before dispatch. */
export function isCanvasWorkflowSurfaceBase(
  value: unknown,
): value is Contract.CanvasWorkflowSurfaceBase {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'file' &&
      Object.hasOwn(value, 'file') &&
      isCanvasSurfaceFileReference(value['file'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'blank' &&
      Object.hasOwn(value, 'width') &&
      typeof value['width'] === 'number' &&
      Number.isSafeInteger(value['width']) &&
      value['width'] >= 1 &&
      value['width'] <= 300000 &&
      Object.hasOwn(value, 'height') &&
      typeof value['height'] === 'number' &&
      Number.isSafeInteger(value['height']) &&
      value['height'] >= 1 &&
      value['height'] <= 300000)
  );
}
/** Validate a bounded CanvasWorkflowMask payload before dispatch. */
export function isCanvasWorkflowMask(value: unknown): value is Contract.CanvasWorkflowMask {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'key') &&
    typeof value['key'] === 'string' &&
    value['key'].length <= 256 &&
    Object.hasOwn(value, 'label') &&
    typeof value['label'] === 'string' &&
    value['label'].length <= 256 &&
    Object.hasOwn(value, 'color') &&
    typeof value['color'] === 'string' &&
    value['color'].length <= 7 &&
    new RegExp('^#[a-fA-F0-9]{6}$').test(value['color']) &&
    Object.hasOwn(value, 'opacity') &&
    typeof value['opacity'] === 'number' &&
    Number.isFinite(value['opacity']) &&
    value['opacity'] >= 0 &&
    value['opacity'] <= 1 &&
    Object.hasOwn(value, 'initial') &&
    (isCanvasSurfaceFileReference(value['initial']) || value['initial'] === null) &&
    Object.hasOwn(value, 'channel') &&
    (value['channel'] === 'red' ||
      value['channel'] === 'green' ||
      value['channel'] === 'blue' ||
      value['channel'] === 'alpha')
  );
}
/** Validate a bounded CanvasWorkflowSurface payload before dispatch. */
export function isCanvasWorkflowSurface(value: unknown): value is Contract.CanvasWorkflowSurface {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'key') &&
    typeof value['key'] === 'string' &&
    value['key'].length <= 256 &&
    Object.hasOwn(value, 'label') &&
    typeof value['label'] === 'string' &&
    value['label'].length <= 256 &&
    Object.hasOwn(value, 'base') &&
    isCanvasWorkflowSurfaceBase(value['base']) &&
    Object.hasOwn(value, 'masks') &&
    Array.isArray(value['masks']) &&
    value['masks'].length <= 32 &&
    value['masks'].every((item: unknown) => isCanvasWorkflowMask(item))
  );
}
/** Validate a bounded CanvasWorkflowAppearance payload before dispatch. */
export function isCanvasWorkflowAppearance(
  value: unknown,
): value is Contract.CanvasWorkflowAppearance {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'surface') &&
    typeof value['surface'] === 'string' &&
    value['surface'].length <= 256 &&
    Object.hasOwn(value, 'mask') &&
    typeof value['mask'] === 'string' &&
    value['mask'].length <= 256 &&
    Object.hasOwn(value, 'color') &&
    typeof value['color'] === 'string' &&
    value['color'].length <= 7 &&
    new RegExp('^#[a-fA-F0-9]{6}$').test(value['color']) &&
    Object.hasOwn(value, 'opacity') &&
    typeof value['opacity'] === 'number' &&
    Number.isFinite(value['opacity']) &&
    value['opacity'] >= 0 &&
    value['opacity'] <= 1
  );
}
/** Validate a bounded CanvasWorkflowResizeMode payload before dispatch. */
export function isCanvasWorkflowResizeMode(
  value: unknown,
): value is Contract.CanvasWorkflowResizeMode {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'canvas' &&
      Object.hasOwn(value, 'anchor') &&
      (value['anchor'] === 'top-left' ||
        value['anchor'] === 'top' ||
        value['anchor'] === 'top-right' ||
        value['anchor'] === 'left' ||
        value['anchor'] === 'center' ||
        value['anchor'] === 'right' ||
        value['anchor'] === 'bottom-left' ||
        value['anchor'] === 'bottom' ||
        value['anchor'] === 'bottom-right')) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'scale' &&
      Object.hasOwn(value, 'resampling') &&
      (value['resampling'] === 'nearest' || value['resampling'] === 'smooth'))
  );
}
/** Validate a bounded CanvasWorkflowCommand payload before dispatch. */
export function isCanvasWorkflowCommand(value: unknown): value is Contract.CanvasWorkflowCommand {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'sync' &&
      Object.hasOwn(value, 'workflow') &&
      typeof value['workflow'] === 'string' &&
      value['workflow'].length <= 256 &&
      Object.hasOwn(value, 'surfaces') &&
      Array.isArray(value['surfaces']) &&
      value['surfaces'].length <= 64 &&
      value['surfaces'].every((item: unknown) => isCanvasWorkflowSurface(item))) ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'activate' &&
      Object.hasOwn(value, 'workflow') &&
      typeof value['workflow'] === 'string' &&
      value['workflow'].length <= 256 &&
      Object.hasOwn(value, 'route') &&
      (value['route'] === 'input' || value['route'] === 'output') &&
      Object.hasOwn(value, 'surface') &&
      ((typeof value['surface'] === 'string' && value['surface'].length <= 256) ||
        value['surface'] === null) &&
      Object.hasOwn(value, 'mask') &&
      ((typeof value['mask'] === 'string' && value['mask'].length <= 256) ||
        value['mask'] === null)) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'appearance' &&
      Object.hasOwn(value, 'workflow') &&
      typeof value['workflow'] === 'string' &&
      value['workflow'].length <= 256 &&
      Object.hasOwn(value, 'masks') &&
      Array.isArray(value['masks']) &&
      value['masks'].length >= 1 &&
      value['masks'].length <= 2048 &&
      value['masks'].every((item: unknown) => isCanvasWorkflowAppearance(item))) ||
    (isRecord(value) &&
      Object.keys(value).length === 6 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'resize' &&
      Object.hasOwn(value, 'workflow') &&
      typeof value['workflow'] === 'string' &&
      value['workflow'].length <= 256 &&
      Object.hasOwn(value, 'surface') &&
      typeof value['surface'] === 'string' &&
      value['surface'].length <= 256 &&
      Object.hasOwn(value, 'width') &&
      typeof value['width'] === 'number' &&
      Number.isSafeInteger(value['width']) &&
      value['width'] >= 1 &&
      value['width'] <= 300000 &&
      Object.hasOwn(value, 'height') &&
      typeof value['height'] === 'number' &&
      Number.isSafeInteger(value['height']) &&
      value['height'] >= 1 &&
      value['height'] <= 300000 &&
      Object.hasOwn(value, 'mode') &&
      isCanvasWorkflowResizeMode(value['mode'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'hover' &&
      Object.hasOwn(value, 'workflow') &&
      typeof value['workflow'] === 'string' &&
      value['workflow'].length <= 256 &&
      Object.hasOwn(value, 'surface') &&
      typeof value['surface'] === 'string' &&
      value['surface'].length <= 256 &&
      Object.hasOwn(value, 'mask') &&
      ((typeof value['mask'] === 'string' && value['mask'].length <= 256) ||
        value['mask'] === null))
  );
}
/** Validate a bounded CanvasWorkflowMaskLayer payload before dispatch. */
export function isCanvasWorkflowMaskLayer(
  value: unknown,
): value is Contract.CanvasWorkflowMaskLayer {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'key') &&
    typeof value['key'] === 'string' &&
    value['key'].length <= 256 &&
    Object.hasOwn(value, 'layerId') &&
    typeof value['layerId'] === 'string' &&
    value['layerId'].length <= 128
  );
}
/** Validate a bounded CanvasWorkflowSyncedSurface payload before dispatch. */
export function isCanvasWorkflowSyncedSurface(
  value: unknown,
): value is Contract.CanvasWorkflowSyncedSurface {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'key') &&
    typeof value['key'] === 'string' &&
    value['key'].length <= 256 &&
    Object.hasOwn(value, 'document') &&
    isCanvasDocument(value['document']) &&
    Object.hasOwn(value, 'masks') &&
    Array.isArray(value['masks']) &&
    value['masks'].length <= 32 &&
    value['masks'].every((item: unknown) => isCanvasWorkflowMaskLayer(item))
  );
}
/** Validate a bounded CanvasWorkflowResult payload before dispatch. */
export function isCanvasWorkflowResult(value: unknown): value is Contract.CanvasWorkflowResult {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'synced' &&
      Object.hasOwn(value, 'surfaces') &&
      Array.isArray(value['surfaces']) &&
      value['surfaces'].length <= 64 &&
      value['surfaces'].every((item: unknown) => isCanvasWorkflowSyncedSurface(item))) ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'activated') ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'applied') ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'failure' &&
      Object.hasOwn(value, 'status') &&
      (value['status'] === 'unavailable' ||
        value['status'] === 'unsupported' ||
        value['status'] === 'stale' ||
        value['status'] === 'cancelled' ||
        value['status'] === 'error') &&
      Object.hasOwn(value, 'message') &&
      typeof value['message'] === 'string' &&
      value['message'].length <= 512)
  );
}
/** Validate a bounded CanvasWorkflowRequest payload before dispatch. */
export function isCanvasWorkflowRequest(value: unknown): value is Contract.CanvasWorkflowRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'workflow.request' &&
    Object.hasOwn(value, 'command') &&
    isCanvasWorkflowCommand(value['command'])
  );
}
/** Validate a bounded CanvasWorkflowReply payload before dispatch. */
export function isCanvasWorkflowReply(value: unknown): value is Contract.CanvasWorkflowReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 3 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'workflow.reply' &&
    Object.hasOwn(value, 'result') &&
    isCanvasWorkflowResult(value['result'])
  );
}
