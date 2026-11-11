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
/** Validate a bounded CanvasSourceTarget payload before dispatch. */
export function isCanvasSourceTarget(value: unknown): value is Contract.CanvasSourceTarget {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'selection') ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'current-layer') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'layer' &&
      Object.hasOwn(value, 'layerId') &&
      typeof value['layerId'] === 'string' &&
      value['layerId'].length <= 128) ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'composition') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'composition-from-layer' &&
      Object.hasOwn(value, 'layerId') &&
      typeof value['layerId'] === 'string' &&
      value['layerId'].length <= 128)
  );
}
/** Validate a bounded CanvasInputTarget payload before dispatch. */
export function isCanvasInputTarget(value: unknown): value is Contract.CanvasInputTarget {
  return (
    isCanvasSourceTarget(value) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'file' &&
      Object.hasOwn(value, 'file') &&
      ((typeof value['file'] === 'string' && value['file'].length <= 4096) ||
        value['file'] === null))
  );
}
/** Validate a bounded CanvasPixelRaster payload before dispatch. */
export function isCanvasPixelRaster(value: unknown): value is Contract.CanvasPixelRaster {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
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
    Object.hasOwn(value, 'format') &&
    (value['format'] === 'rgba8' || value['format'] === 'coverage8') &&
    Object.hasOwn(value, 'data') &&
    typeof value['data'] === 'string' &&
    value['data'].length <= 89478488
  );
}
/** Validate a bounded CanvasSourceLayer payload before dispatch. */
export function isCanvasSourceLayer(value: unknown): value is Contract.CanvasSourceLayer {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 128 &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 256 &&
    Object.hasOwn(value, 'parentId') &&
    ((typeof value['parentId'] === 'string' && value['parentId'].length <= 128) ||
      value['parentId'] === null) &&
    Object.hasOwn(value, 'depth') &&
    typeof value['depth'] === 'number' &&
    Number.isSafeInteger(value['depth']) &&
    value['depth'] >= 0 &&
    value['depth'] <= 128 &&
    Object.hasOwn(value, 'visible') &&
    typeof value['visible'] === 'boolean'
  );
}
/** Validate a bounded CanvasSourceContext payload before dispatch. */
export function isCanvasSourceContext(value: unknown): value is Contract.CanvasSourceContext {
  return (
    isRecord(value) &&
    Object.keys(value).length === 8 &&
    Object.hasOwn(value, 'kind') &&
    value['kind'] === 'context' &&
    Object.hasOwn(value, 'supported') &&
    typeof value['supported'] === 'boolean' &&
    Object.hasOwn(value, 'document') &&
    (isCanvasDocument(value['document']) || value['document'] === null) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'string' &&
    value['revision'].length <= 512 &&
    Object.hasOwn(value, 'targets') &&
    Array.isArray(value['targets']) &&
    value['targets'].length <= 5 &&
    value['targets'].every(
      (item: unknown) =>
        item === 'selection' ||
        item === 'current-layer' ||
        item === 'layer' ||
        item === 'composition' ||
        item === 'composition-from-layer',
    ) &&
    Object.hasOwn(value, 'layers') &&
    Array.isArray(value['layers']) &&
    value['layers'].length <= 4096 &&
    value['layers'].every((item: unknown) => isCanvasSourceLayer(item)) &&
    Object.hasOwn(value, 'currentLayerId') &&
    ((typeof value['currentLayerId'] === 'string' && value['currentLayerId'].length <= 128) ||
      value['currentLayerId'] === null) &&
    Object.hasOwn(value, 'observesChanges') &&
    typeof value['observesChanges'] === 'boolean'
  );
}
/** Validate a bounded CanvasSourceSnapshot payload before dispatch. */
export function isCanvasSourceSnapshot(value: unknown): value is Contract.CanvasSourceSnapshot {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'target') &&
    isCanvasSourceTarget(value['target']) &&
    Object.hasOwn(value, 'document') &&
    isCanvasDocument(value['document']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'string' &&
    value['revision'].length <= 512 &&
    Object.hasOwn(value, 'pixels') &&
    isCanvasPixelRaster(value['pixels'])
  );
}
/** Validate a bounded CanvasSourceCommand payload before dispatch. */
export function isCanvasSourceCommand(value: unknown): value is Contract.CanvasSourceCommand {
  return (
    isCanvasSourceQuery(value) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'watch' &&
      Object.hasOwn(value, 'enabled') &&
      typeof value['enabled'] === 'boolean')
  );
}
/** Validate a bounded CanvasSourceResult payload before dispatch. */
export function isCanvasSourceResult(value: unknown): value is Contract.CanvasSourceResult {
  return (
    isCanvasSourceContext(value) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'captured' &&
      Object.hasOwn(value, 'snapshots') &&
      Array.isArray(value['snapshots']) &&
      value['snapshots'].length <= 64 &&
      value['snapshots'].every((item: unknown) => isCanvasSourceSnapshot(item))) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'watching' &&
      Object.hasOwn(value, 'supported') &&
      typeof value['supported'] === 'boolean') ||
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
/** Validate a bounded CanvasSourceRequest payload before dispatch. */
export function isCanvasSourceRequest(value: unknown): value is Contract.CanvasSourceRequest {
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
    value['type'] === 'source.request' &&
    Object.hasOwn(value, 'command') &&
    isCanvasSourceCommand(value['command'])
  );
}
/** Validate a bounded CanvasSourceReply payload before dispatch. */
export function isCanvasSourceReply(value: unknown): value is Contract.CanvasSourceReply {
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
    value['type'] === 'source.reply' &&
    Object.hasOwn(value, 'result') &&
    isCanvasSourceResult(value['result'])
  );
}
/** Validate a bounded CanvasSourceChanged payload before dispatch. */
export function isCanvasSourceChanged(value: unknown): value is Contract.CanvasSourceChanged {
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
    value['requestId'] === 0 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'source.changed' &&
    Object.hasOwn(value, 'change') &&
    isCanvasSourceChange(value['change'])
  );
}
/** Validate a bounded CanvasSourceChange payload before dispatch. */
export function isCanvasSourceChange(value: unknown): value is Contract.CanvasSourceChange {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'documentId') &&
    ((typeof value['documentId'] === 'string' && value['documentId'].length <= 128) ||
      value['documentId'] === null) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'string' &&
    value['revision'].length <= 512
  );
}
/** Validate a bounded CanvasSourceQuery payload before dispatch. */
export function isCanvasSourceQuery(value: unknown): value is Contract.CanvasSourceQuery {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'context') ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'capture' &&
      Object.hasOwn(value, 'document') &&
      isCanvasDocument(value['document']) &&
      Object.hasOwn(value, 'targets') &&
      Array.isArray(value['targets']) &&
      value['targets'].length <= 64 &&
      value['targets'].every((item: unknown) => isCanvasSourceTarget(item)) &&
      Object.hasOwn(value, 'maximumSize') &&
      typeof value['maximumSize'] === 'number' &&
      Number.isSafeInteger(value['maximumSize']) &&
      value['maximumSize'] >= 0 &&
      value['maximumSize'] <= 1024)
  );
}
/** Validate a bounded CanvasInputSource payload before dispatch. */
export function isCanvasInputSource(value: unknown): value is Contract.CanvasInputSource {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'file' &&
      Object.hasOwn(value, 'file') &&
      ((typeof value['file'] === 'string' && value['file'].length <= 4096) ||
        value['file'] === null)) ||
    (isRecord(value) &&
      Object.keys(value).length === 4 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'canvas' &&
      Object.hasOwn(value, 'canvasId') &&
      typeof value['canvasId'] === 'string' &&
      value['canvasId'].length <= 128 &&
      Object.hasOwn(value, 'documentId') &&
      ((typeof value['documentId'] === 'string' && value['documentId'].length <= 128) ||
        value['documentId'] === null) &&
      Object.hasOwn(value, 'target') &&
      isCanvasSourceTarget(value['target']))
  );
}
