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
/** Generated protocol validation; edit the authoritative masks.schema.json. */
import type * as Contract from '../contracts/CanvasMasksProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded CanvasDocument payload before dispatch. */
export function isCanvasDocument(value: unknown): value is Contract.CanvasDocument {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 128 &&
    Object.hasOwn(value, 'geometryRevision') &&
    typeof value['geometryRevision'] === 'string' &&
    value['geometryRevision'].length <= 512 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isSafeInteger(value['width']) &&
    value['width'] >= 1 &&
    value['width'] <= 300000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isSafeInteger(value['height']) &&
    value['height'] >= 1 &&
    value['height'] <= 300000
  );
}
/** Validate a bounded CanvasMaskPixels payload before dispatch. */
export function isCanvasMaskPixels(value: unknown): value is Contract.CanvasMaskPixels {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isSafeInteger(value['width']) &&
    value['width'] >= 1 &&
    value['width'] <= 65536 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isSafeInteger(value['height']) &&
    value['height'] >= 1 &&
    value['height'] <= 65536 &&
    Object.hasOwn(value, 'data') &&
    typeof value['data'] === 'string' &&
    value['data'].length <= 89478488
  );
}
/** Validate a bounded CanvasMaskReference payload before dispatch. */
export function isCanvasMaskReference(value: unknown): value is Contract.CanvasMaskReference {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'canvasId') &&
    typeof value['canvasId'] === 'string' &&
    value['canvasId'].length <= 128 &&
    Object.hasOwn(value, 'document') &&
    isCanvasDocument(value['document']) &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 128
  );
}
/** Validate a bounded CanvasMaskSnapshot payload before dispatch. */
export function isCanvasMaskSnapshot(value: unknown): value is Contract.CanvasMaskSnapshot {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'mask') &&
    isCanvasMaskReference(value['mask']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'string' &&
    value['revision'].length <= 512 &&
    Object.hasOwn(value, 'pixels') &&
    isCanvasMaskPixels(value['pixels'])
  );
}
/** Validate a bounded CanvasMaskContext payload before dispatch. */
export function isCanvasMaskContext(value: unknown): value is Contract.CanvasMaskContext {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'kind') &&
    value['kind'] === 'context' &&
    Object.hasOwn(value, 'supported') &&
    typeof value['supported'] === 'boolean' &&
    Object.hasOwn(value, 'document') &&
    (isCanvasDocument(value['document']) || value['document'] === null) &&
    Object.hasOwn(value, 'observesChanges') &&
    typeof value['observesChanges'] === 'boolean'
  );
}
/** Validate a bounded CanvasMaskCreate payload before dispatch. */
export function isCanvasMaskCreate(value: unknown): value is Contract.CanvasMaskCreate {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'create' &&
    Object.hasOwn(value, 'document') &&
    isCanvasDocument(value['document']) &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 128 &&
    Object.hasOwn(value, 'source') &&
    (value['source'] === 'selection' ||
      value['source'] === 'blank' ||
      value['source'] === 'pixels') &&
    Object.hasOwn(value, 'pixels') &&
    (isCanvasMaskPixels(value['pixels']) || value['pixels'] === null)
  );
}
/** Validate a bounded CanvasMaskActivate payload before dispatch. */
export function isCanvasMaskActivate(value: unknown): value is Contract.CanvasMaskActivate {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'activate' &&
    Object.hasOwn(value, 'mask') &&
    isCanvasMaskReference(value['mask'])
  );
}
/** Validate a bounded CanvasMaskCapture payload before dispatch. */
export function isCanvasMaskCapture(value: unknown): value is Contract.CanvasMaskCapture {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'capture' &&
    Object.hasOwn(value, 'masks') &&
    Array.isArray(value['masks']) &&
    value['masks'].length <= 64 &&
    value['masks'].every((item: unknown) => isCanvasMaskReference(item)) &&
    Object.hasOwn(value, 'maximumSize') &&
    typeof value['maximumSize'] === 'number' &&
    Number.isSafeInteger(value['maximumSize']) &&
    value['maximumSize'] >= 0 &&
    value['maximumSize'] <= 1024
  );
}
/** Validate a bounded CanvasMaskCommand payload before dispatch. */
export function isCanvasMaskCommand(value: unknown): value is Contract.CanvasMaskCommand {
  return (
    isCanvasMaskQuery(value) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'enabled') &&
      typeof value['enabled'] === 'boolean' &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'watch')
  );
}
/** Validate a bounded CanvasMaskResult payload before dispatch. */
export function isCanvasMaskResult(value: unknown): value is Contract.CanvasMaskResult {
  return (
    isCanvasMaskContext(value) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'created' &&
      Object.hasOwn(value, 'mask') &&
      isCanvasMaskReference(value['mask'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'activated') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'captured' &&
      Object.hasOwn(value, 'snapshots') &&
      Array.isArray(value['snapshots']) &&
      value['snapshots'].length <= 64 &&
      value['snapshots'].every((item: unknown) => isCanvasMaskSnapshot(item))) ||
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
      value['message'].length <= 512) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'supported') &&
      typeof value['supported'] === 'boolean' &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'watching')
  );
}
/** Validate a bounded CanvasMaskRequest payload before dispatch. */
export function isCanvasMaskRequest(value: unknown): value is Contract.CanvasMaskRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas.masks' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'command') &&
    isCanvasMaskCommand(value['command'])
  );
}
/** Validate a bounded CanvasMaskReply payload before dispatch. */
export function isCanvasMaskReply(value: unknown): value is Contract.CanvasMaskReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas.masks' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'requestId') &&
    typeof value['requestId'] === 'number' &&
    Number.isSafeInteger(value['requestId']) &&
    value['requestId'] >= 1 &&
    value['requestId'] <= 2147483647 &&
    Object.hasOwn(value, 'result') &&
    isCanvasMaskResult(value['result'])
  );
}
/** Validate a bounded CanvasMaskChanged payload before dispatch. */
export function isCanvasMaskChanged(value: unknown): value is Contract.CanvasMaskChanged {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.canvas.masks' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'changed' &&
    Object.hasOwn(value, 'documentId') &&
    typeof value['documentId'] === 'string' &&
    value['documentId'].length <= 128 &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'string' &&
    value['revision'].length <= 512 &&
    Object.hasOwn(value, 'maskId') &&
    typeof value['maskId'] === 'string' &&
    value['maskId'].length <= 128
  );
}
/** Validate a bounded CanvasMaskQuery payload before dispatch. */
export function isCanvasMaskQuery(value: unknown): value is Contract.CanvasMaskQuery {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'context') ||
    isCanvasMaskCreate(value) ||
    isCanvasMaskActivate(value) ||
    isCanvasMaskCapture(value) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'release' &&
      Object.hasOwn(value, 'mask') &&
      (isCanvasMaskReference(value['mask']) || value['mask'] === null))
  );
}
/** Validate a bounded CanvasMaskMessage payload before dispatch. */
export function isCanvasMaskMessage(value: unknown): value is Contract.CanvasMaskMessage {
  return isCanvasMaskRequest(value) || isCanvasMaskReply(value) || isCanvasMaskChanged(value);
}
