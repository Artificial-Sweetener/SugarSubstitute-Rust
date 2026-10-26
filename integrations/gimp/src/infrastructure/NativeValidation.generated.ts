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
/** Generated protocol validation; edit the authoritative integrations/gimp/native.schema.json. */
import type * as Contract from '../contracts/NativeProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded NativeId payload before dispatch. */
export function isNativeId(value: unknown): value is Contract.NativeId {
  return (
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 2147483647
  );
}
/** Validate a bounded NativeTattoo payload before dispatch. */
export function isNativeTattoo(value: unknown): value is Contract.NativeTattoo {
  return (
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 4294967295
  );
}
/** Validate a bounded NativeToken payload before dispatch. */
export function isNativeToken(value: unknown): value is Contract.NativeToken {
  return (
    typeof value === 'string' &&
    value.length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]+(?![\\s\\S])').test(value)
  );
}
/** Validate a bounded NativeDocument payload before dispatch. */
export function isNativeDocument(value: unknown): value is Contract.NativeDocument {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'imageId') &&
    isNativeId(value['imageId']) &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'string' &&
    value['revision'].length <= 512 &&
    Object.hasOwn(value, 'width') &&
    isCanvasWidth(value['width']) &&
    Object.hasOwn(value, 'height') &&
    isCanvasHeight(value['height']) &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 256
  );
}
/** Validate a bounded NativeLayer payload before dispatch. */
export function isNativeLayer(value: unknown): value is Contract.NativeLayer {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'tattoo') &&
    isNativeTattoo(value['tattoo']) &&
    Object.hasOwn(value, 'parentTattoo') &&
    (isNativeTattoo(value['parentTattoo']) || value['parentTattoo'] === null) &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 256 &&
    Object.hasOwn(value, 'depth') &&
    typeof value['depth'] === 'number' &&
    Number.isSafeInteger(value['depth']) &&
    value['depth'] >= 0 &&
    value['depth'] <= 128 &&
    Object.hasOwn(value, 'visible') &&
    typeof value['visible'] === 'boolean' &&
    Object.hasOwn(value, 'hasMask') &&
    typeof value['hasMask'] === 'boolean'
  );
}
/** Validate a bounded NativeLayers payload before dispatch. */
export function isNativeLayers(value: unknown): value is Contract.NativeLayers {
  return (
    Array.isArray(value) &&
    value.length <= 4096 &&
    value.every((item: unknown) => isNativeLayer(item))
  );
}
/** Validate a bounded NativeVisibility payload before dispatch. */
export function isNativeVisibility(value: unknown): value is Contract.NativeVisibility {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'tattoo') &&
    isNativeTattoo(value['tattoo']) &&
    Object.hasOwn(value, 'visible') &&
    typeof value['visible'] === 'boolean'
  );
}
/** Validate a bounded NativeCommand payload before dispatch. */
export function isNativeCommand(value: unknown): value is Contract.NativeCommand {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'context') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'snapshot.begin' &&
      Object.hasOwn(value, 'document') &&
      isNativeDocument(value['document'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'snapshot.release' &&
      Object.hasOwn(value, 'token') &&
      isNativeToken(value['token'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'snapshot.selection' &&
      Object.hasOwn(value, 'token') &&
      isNativeToken(value['token']) &&
      Object.hasOwn(value, 'width') &&
      isRasterWidth(value['width']) &&
      Object.hasOwn(value, 'height') &&
      isRasterHeight(value['height']) &&
      Object.hasOwn(value, 'scale') &&
      typeof value['scale'] === 'number' &&
      Number.isFinite(value['scale']) &&
      value['scale'] >= 0 &&
      value['scale'] <= 1) ||
    (isRecord(value) &&
      Object.keys(value).length === 6 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'snapshot.projection' &&
      Object.hasOwn(value, 'token') &&
      isNativeToken(value['token']) &&
      Object.hasOwn(value, 'width') &&
      isRasterWidth(value['width']) &&
      Object.hasOwn(value, 'height') &&
      isRasterHeight(value['height']) &&
      Object.hasOwn(value, 'scale') &&
      typeof value['scale'] === 'number' &&
      Number.isFinite(value['scale']) &&
      value['scale'] >= 0 &&
      value['scale'] <= 1 &&
      Object.hasOwn(value, 'visibility') &&
      Array.isArray(value['visibility']) &&
      value['visibility'].length <= 4096 &&
      value['visibility'].every((item: unknown) => isNativeVisibility(item))) ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'channel.create' &&
      Object.hasOwn(value, 'document') &&
      isNativeDocument(value['document']) &&
      Object.hasOwn(value, 'name') &&
      typeof value['name'] === 'string' &&
      value['name'].length <= 256 &&
      Object.hasOwn(value, 'source') &&
      (value['source'] === 'selection' ||
        value['source'] === 'blank' ||
        value['source'] === 'pixels') &&
      Object.hasOwn(value, 'data') &&
      (isRasterData(value['data']) || value['data'] === null)) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'channel.select' &&
      Object.hasOwn(value, 'document') &&
      isNativeDocument(value['document']) &&
      Object.hasOwn(value, 'tattoo') &&
      isNativeTattoo(value['tattoo'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'channel.release' &&
      Object.hasOwn(value, 'tattoo') &&
      (isNativeTattoo(value['tattoo']) || value['tattoo'] === null)) ||
    (isRecord(value) &&
      Object.keys(value).length === 6 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'snapshot.channel' &&
      Object.hasOwn(value, 'token') &&
      isNativeToken(value['token']) &&
      Object.hasOwn(value, 'tattoo') &&
      isNativeTattoo(value['tattoo']) &&
      Object.hasOwn(value, 'width') &&
      isRasterWidth(value['width']) &&
      Object.hasOwn(value, 'height') &&
      isRasterHeight(value['height']) &&
      Object.hasOwn(value, 'scale') &&
      typeof value['scale'] === 'number' &&
      Number.isFinite(value['scale']) &&
      value['scale'] >= 0 &&
      value['scale'] <= 1) ||
    (isRecord(value) &&
      Object.keys(value).length === 8 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'image.insert' &&
      Object.hasOwn(value, 'document') &&
      isNativeDocument(value['document']) &&
      Object.hasOwn(value, 'data') &&
      isPngData(value['data']) &&
      Object.hasOwn(value, 'width') &&
      typeof value['width'] === 'number' &&
      Number.isSafeInteger(value['width']) &&
      value['width'] >= 1 &&
      value['width'] <= 30000 &&
      Object.hasOwn(value, 'height') &&
      typeof value['height'] === 'number' &&
      Number.isSafeInteger(value['height']) &&
      value['height'] >= 1 &&
      value['height'] <= 30000 &&
      Object.hasOwn(value, 'name') &&
      typeof value['name'] === 'string' &&
      value['name'].length <= 256 &&
      Object.hasOwn(value, 'left') &&
      typeof value['left'] === 'number' &&
      Number.isSafeInteger(value['left']) &&
      value['left'] >= -300000 &&
      value['left'] <= 300000 &&
      Object.hasOwn(value, 'top') &&
      typeof value['top'] === 'number' &&
      Number.isSafeInteger(value['top']) &&
      value['top'] >= -300000 &&
      value['top'] <= 300000)
  );
}
/** Validate a bounded NativeResult payload before dispatch. */
export function isNativeResult(value: unknown): value is Contract.NativeResult {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'channel-created' &&
      Object.hasOwn(value, 'tattoo') &&
      isNativeTattoo(value['tattoo']) &&
      Object.hasOwn(value, 'committed') &&
      value['committed'] === true) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'channel-selected' &&
      Object.hasOwn(value, 'committed') &&
      value['committed'] === true) ||
    (isRecord(value) &&
      Object.keys(value).length === 6 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'context' &&
      Object.hasOwn(value, 'hostId') &&
      isNativeToken(value['hostId']) &&
      Object.hasOwn(value, 'document') &&
      (isNativeDocument(value['document']) || value['document'] === null) &&
      Object.hasOwn(value, 'layers') &&
      isNativeLayers(value['layers']) &&
      Object.hasOwn(value, 'primaryLayer') &&
      (isNativeTattoo(value['primaryLayer']) || value['primaryLayer'] === null) &&
      Object.hasOwn(value, 'rgb') &&
      typeof value['rgb'] === 'boolean') ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'snapshot' &&
      Object.hasOwn(value, 'token') &&
      isNativeToken(value['token']) &&
      Object.hasOwn(value, 'document') &&
      isNativeDocument(value['document']) &&
      Object.hasOwn(value, 'layers') &&
      isNativeLayers(value['layers']) &&
      Object.hasOwn(value, 'primaryLayer') &&
      (isNativeTattoo(value['primaryLayer']) || value['primaryLayer'] === null)) ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'released') ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'pixels' &&
      Object.hasOwn(value, 'width') &&
      isRasterWidth(value['width']) &&
      Object.hasOwn(value, 'height') &&
      isRasterHeight(value['height']) &&
      Object.hasOwn(value, 'format') &&
      (value['format'] === 'rgba8' || value['format'] === 'coverage8') &&
      Object.hasOwn(value, 'data') &&
      isRasterData(value['data'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 7 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'inserted' &&
      Object.hasOwn(value, 'committed') &&
      value['committed'] === true &&
      Object.hasOwn(value, 'tattoo') &&
      isNativeTattoo(value['tattoo']) &&
      Object.hasOwn(value, 'left') &&
      typeof value['left'] === 'number' &&
      Number.isSafeInteger(value['left']) &&
      value['left'] >= -300000 &&
      value['left'] <= 300000 &&
      Object.hasOwn(value, 'top') &&
      typeof value['top'] === 'number' &&
      Number.isSafeInteger(value['top']) &&
      value['top'] >= -300000 &&
      value['top'] <= 300000 &&
      Object.hasOwn(value, 'width') &&
      typeof value['width'] === 'number' &&
      Number.isSafeInteger(value['width']) &&
      value['width'] >= 1 &&
      value['width'] <= 30000 &&
      Object.hasOwn(value, 'height') &&
      typeof value['height'] === 'number' &&
      Number.isSafeInteger(value['height']) &&
      value['height'] >= 1 &&
      value['height'] <= 30000) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'failure' &&
      Object.hasOwn(value, 'status') &&
      (value['status'] === 'cancelled' ||
        value['status'] === 'stale' ||
        value['status'] === 'unsupported' ||
        value['status'] === 'error') &&
      Object.hasOwn(value, 'message') &&
      typeof value['message'] === 'string' &&
      value['message'].length <= 512)
  );
}
/** Validate a bounded NativeRequest payload before dispatch. */
export function isNativeRequest(value: unknown): value is Contract.NativeRequest {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'request' &&
    Object.hasOwn(value, 'id') &&
    isNativeId(value['id']) &&
    Object.hasOwn(value, 'command') &&
    isNativeCommand(value['command'])
  );
}
/** Validate a bounded NativeCancel payload before dispatch. */
export function isNativeCancel(value: unknown): value is Contract.NativeCancel {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'cancel' &&
    Object.hasOwn(value, 'id') &&
    isNativeId(value['id'])
  );
}
/** Validate a bounded NativeReply payload before dispatch. */
export function isNativeReply(value: unknown): value is Contract.NativeReply {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'reply' &&
    Object.hasOwn(value, 'id') &&
    isNativeId(value['id']) &&
    Object.hasOwn(value, 'result') &&
    isNativeResult(value['result'])
  );
}
/** Validate a bounded CanvasWidth payload before dispatch. */
export function isCanvasWidth(value: unknown): value is Contract.CanvasWidth {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 300000;
}
/** Validate a bounded CanvasHeight payload before dispatch. */
export function isCanvasHeight(value: unknown): value is Contract.CanvasHeight {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 300000;
}
/** Validate a bounded RasterWidth payload before dispatch. */
export function isRasterWidth(value: unknown): value is Contract.RasterWidth {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 65536;
}
/** Validate a bounded RasterHeight payload before dispatch. */
export function isRasterHeight(value: unknown): value is Contract.RasterHeight {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= 65536;
}
/** Validate a bounded PngData payload before dispatch. */
export function isPngData(value: unknown): value is Contract.PngData {
  return typeof value === 'string' && value.length <= 89478488;
}
/** Validate a bounded RasterData payload before dispatch. */
export function isRasterData(value: unknown): value is Contract.RasterData {
  return typeof value === 'string' && value.length <= 89478488;
}
/** Validate a bounded NativeMessage payload before dispatch. */
export function isNativeMessage(value: unknown): value is Contract.NativeMessage {
  return isNativeRequest(value) || isNativeCancel(value) || isNativeReply(value);
}
