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

import { isRecord } from './ContractRecord.js';
/** Validate a bounded CanvasToolId payload before dispatch. */
export function isCanvasToolId(value: unknown): value is Contract.CanvasToolId {
  return (
    value === 'pan-zoom' ||
    value === 'move' ||
    value === 'brush' ||
    value === 'eraser' ||
    value === 'select-rectangle' ||
    value === 'select-ellipse' ||
    value === 'select-lasso' ||
    value === 'mask-rectangle' ||
    value === 'mask-ellipse' ||
    value === 'mask-lasso' ||
    value === 'transform' ||
    value === 'shared-edges'
  );
}
/** Validate a bounded CanvasBrushOptions payload before dispatch. */
export function isCanvasBrushOptions(value: unknown): value is Contract.CanvasBrushOptions {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'size') &&
    typeof value['size'] === 'number' &&
    Number.isSafeInteger(value['size']) &&
    value['size'] >= 1 &&
    value['size'] <= 1000 &&
    Object.hasOwn(value, 'hardness') &&
    typeof value['hardness'] === 'number' &&
    Number.isSafeInteger(value['hardness']) &&
    value['hardness'] >= 0 &&
    value['hardness'] <= 100 &&
    Object.hasOwn(value, 'opacity') &&
    typeof value['opacity'] === 'number' &&
    Number.isSafeInteger(value['opacity']) &&
    value['opacity'] >= 0 &&
    value['opacity'] <= 100
  );
}
/** Validate a bounded CanvasEditCommand payload before dispatch. */
export function isCanvasEditCommand(value: unknown): value is Contract.CanvasEditCommand {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'state') ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'tool' &&
      Object.hasOwn(value, 'tool') &&
      isCanvasToolId(value['tool'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'options' &&
      Object.hasOwn(value, 'tool') &&
      (value['tool'] === 'brush' || value['tool'] === 'eraser') &&
      Object.hasOwn(value, 'options') &&
      isCanvasBrushOptions(value['options'])) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'history' &&
      Object.hasOwn(value, 'action') &&
      (value['action'] === 'undo' || value['action'] === 'redo')) ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'commit') ||
    (isRecord(value) &&
      Object.keys(value).length === 1 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'cancel') ||
    (isRecord(value) &&
      Object.keys(value).length === 3 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'nudge' &&
      Object.hasOwn(value, 'dx') &&
      typeof value['dx'] === 'number' &&
      Number.isSafeInteger(value['dx']) &&
      value['dx'] >= -100 &&
      value['dx'] <= 100 &&
      Object.hasOwn(value, 'dy') &&
      typeof value['dy'] === 'number' &&
      Number.isSafeInteger(value['dy']) &&
      value['dy'] >= -100 &&
      value['dy'] <= 100) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'selection' &&
      Object.hasOwn(value, 'action') &&
      (value['action'] === 'deselect' || value['action'] === 'clear')) ||
    (isRecord(value) &&
      Object.keys(value).length === 5 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'modify' &&
      Object.hasOwn(value, 'target') &&
      (value['target'] === 'selection' || value['target'] === 'mask') &&
      Object.hasOwn(value, 'operation') &&
      (value['operation'] === 'expand' ||
        value['operation'] === 'contract' ||
        value['operation'] === 'feather') &&
      Object.hasOwn(value, 'pixels') &&
      typeof value['pixels'] === 'number' &&
      Number.isSafeInteger(value['pixels']) &&
      value['pixels'] >= 1 &&
      value['pixels'] <= 999 &&
      Object.hasOwn(value, 'phase') &&
      (value['phase'] === 'preview' ||
        value['phase'] === 'apply' ||
        value['phase'] === 'cancel')) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'mode' &&
      Object.hasOwn(value, 'mode') &&
      (value['mode'] === 'none' || value['mode'] === 'coverage')) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'transform' &&
      Object.hasOwn(value, 'action') &&
      (value['action'] === 'rotate-left' ||
        value['action'] === 'rotate-right' ||
        value['action'] === 'flip-horizontal' ||
        value['action'] === 'flip-vertical')) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'session' &&
      Object.hasOwn(value, 'action') &&
      (value['action'] === 'undo' ||
        value['action'] === 'redo' ||
        value['action'] === 'apply' ||
        value['action'] === 'cancel')) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'type') &&
      value['type'] === 'navigation' &&
      Object.hasOwn(value, 'held') &&
      typeof value['held'] === 'boolean')
  );
}
/** Validate a bounded CanvasEditTool payload before dispatch. */
export function isCanvasEditTool(value: unknown): value is Contract.CanvasEditTool {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'id') &&
    isCanvasToolId(value['id']) &&
    Object.hasOwn(value, 'available') &&
    typeof value['available'] === 'boolean'
  );
}
/** Validate a bounded CanvasEditToolOptions payload before dispatch. */
export function isCanvasEditToolOptions(value: unknown): value is Contract.CanvasEditToolOptions {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'brush') &&
    isCanvasBrushOptions(value['brush']) &&
    Object.hasOwn(value, 'eraser') &&
    isCanvasBrushOptions(value['eraser'])
  );
}
/** Validate a bounded CanvasEditSelectionBounds payload before dispatch. */
export function isCanvasEditSelectionBounds(
  value: unknown,
): value is Contract.CanvasEditSelectionBounds {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'left') &&
    typeof value['left'] === 'number' &&
    Number.isFinite(value['left']) &&
    value['left'] >= -1000000 &&
    value['left'] <= 1000000 &&
    Object.hasOwn(value, 'top') &&
    typeof value['top'] === 'number' &&
    Number.isFinite(value['top']) &&
    value['top'] >= -1000000 &&
    value['top'] <= 1000000 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isFinite(value['width']) &&
    value['width'] > 0 &&
    value['width'] <= 1000000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isFinite(value['height']) &&
    value['height'] > 0 &&
    value['height'] <= 1000000
  );
}
/** Validate a bounded CanvasEditSelection payload before dispatch. */
export function isCanvasEditSelection(value: unknown): value is Contract.CanvasEditSelection {
  return (
    isRecord(value) &&
    Object.keys(value).length === 1 &&
    Object.hasOwn(value, 'bounds') &&
    isCanvasEditSelectionBounds(value['bounds'])
  );
}
/** Validate a bounded CanvasEditHistory payload before dispatch. */
export function isCanvasEditHistory(value: unknown): value is Contract.CanvasEditHistory {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'undo') &&
    typeof value['undo'] === 'boolean' &&
    Object.hasOwn(value, 'redo') &&
    typeof value['redo'] === 'boolean'
  );
}
/** Validate a bounded CanvasEditSession payload before dispatch. */
export function isCanvasEditSession(value: unknown): value is Contract.CanvasEditSession {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'kind') &&
    (value['kind'] === 'move' ||
      value['kind'] === 'transform' ||
      value['kind'] === 'shared-edges' ||
      value['kind'] === 'modify') &&
    Object.hasOwn(value, 'undo') &&
    typeof value['undo'] === 'boolean' &&
    Object.hasOwn(value, 'redo') &&
    typeof value['redo'] === 'boolean'
  );
}
/** Validate a bounded CanvasEditState payload before dispatch. */
export function isCanvasEditState(value: unknown): value is Contract.CanvasEditState {
  return (
    isRecord(value) &&
    Object.keys(value).length === 10 &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 0 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'document') &&
    ((typeof value['document'] === 'string' && value['document'].length <= 128) ||
      value['document'] === null) &&
    Object.hasOwn(value, 'activeMask') &&
    ((typeof value['activeMask'] === 'string' && value['activeMask'].length <= 128) ||
      value['activeMask'] === null) &&
    Object.hasOwn(value, 'tool') &&
    isCanvasToolId(value['tool']) &&
    Object.hasOwn(value, 'tools') &&
    Array.isArray(value['tools']) &&
    value['tools'].length <= 12 &&
    value['tools'].every((item: unknown) => isCanvasEditTool(item)) &&
    Object.hasOwn(value, 'options') &&
    isCanvasEditToolOptions(value['options']) &&
    Object.hasOwn(value, 'selection') &&
    (isCanvasEditSelection(value['selection']) || value['selection'] === null) &&
    Object.hasOwn(value, 'history') &&
    isCanvasEditHistory(value['history']) &&
    Object.hasOwn(value, 'session') &&
    (isCanvasEditSession(value['session']) || value['session'] === null) &&
    Object.hasOwn(value, 'mode') &&
    (value['mode'] === 'none' || value['mode'] === 'coverage')
  );
}
/** Validate a bounded CanvasEditResult payload before dispatch. */
export function isCanvasEditResult(value: unknown): value is Contract.CanvasEditResult {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'kind') &&
      value['kind'] === 'state' &&
      Object.hasOwn(value, 'state') &&
      isCanvasEditState(value['state'])) ||
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
/** Validate a bounded CanvasEditRequest payload before dispatch. */
export function isCanvasEditRequest(value: unknown): value is Contract.CanvasEditRequest {
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
    value['type'] === 'edit.request' &&
    Object.hasOwn(value, 'command') &&
    isCanvasEditCommand(value['command'])
  );
}
/** Validate a bounded CanvasEditReply payload before dispatch. */
export function isCanvasEditReply(value: unknown): value is Contract.CanvasEditReply {
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
    value['type'] === 'edit.reply' &&
    Object.hasOwn(value, 'result') &&
    isCanvasEditResult(value['result'])
  );
}
/** Validate a bounded CanvasEditChanged payload before dispatch. */
export function isCanvasEditChanged(value: unknown): value is Contract.CanvasEditChanged {
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
    value['type'] === 'edit.changed' &&
    Object.hasOwn(value, 'state') &&
    isCanvasEditState(value['state'])
  );
}
