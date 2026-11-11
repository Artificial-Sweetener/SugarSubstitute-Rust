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
/** Generated protocol validation; edit the authoritative session.schema.json. */
import type * as Contract from '../contracts/SessionProtocol.generated.js';

import { isRecord } from './ContractRecord.js';
/** Validate a bounded SessionSelection payload before dispatch. */
export function isSessionSelection(value: unknown): value is Contract.SessionSelection {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'outputId') &&
    typeof value['outputId'] === 'string' &&
    value['outputId'].length <= 36 &&
    Object.hasOwn(value, 'sourceId') &&
    typeof value['sourceId'] === 'string' &&
    value['sourceId'].length <= 36 &&
    Object.hasOwn(value, 'sceneId') &&
    ((typeof value['sceneId'] === 'string' && value['sceneId'].length <= 36) ||
      value['sceneId'] === null) &&
    Object.hasOwn(value, 'batchIndex') &&
    typeof value['batchIndex'] === 'number' &&
    Number.isSafeInteger(value['batchIndex']) &&
    value['batchIndex'] >= 0 &&
    value['batchIndex'] <= 8191 &&
    Object.hasOwn(value, 'listIndex') &&
    typeof value['listIndex'] === 'number' &&
    Number.isSafeInteger(value['listIndex']) &&
    value['listIndex'] >= 0 &&
    value['listIndex'] <= 8191
  );
}
/** Validate a bounded SessionCamera payload before dispatch. */
export function isSessionCamera(value: unknown): value is Contract.SessionCamera {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'outputId') &&
    typeof value['outputId'] === 'string' &&
    value['outputId'].length <= 36 &&
    Object.hasOwn(value, 'inspection') &&
    isSessionInspection(value['inspection'])
  );
}
/** Validate a bounded SessionComparison payload before dispatch. */
export function isSessionComparison(value: unknown): value is Contract.SessionComparison {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'enabled') &&
    typeof value['enabled'] === 'boolean' &&
    Object.hasOwn(value, 'selection') &&
    (isSessionSelection(value['selection']) || value['selection'] === null) &&
    Object.hasOwn(value, 'orientation') &&
    (value['orientation'] === 'horizontal' || value['orientation'] === 'vertical') &&
    Object.hasOwn(value, 'split') &&
    typeof value['split'] === 'number' &&
    Number.isFinite(value['split']) &&
    value['split'] >= 0 &&
    value['split'] <= 1
  );
}
/** Validate a bounded SessionView payload before dispatch. */
export function isSessionView(value: unknown): value is Contract.SessionView {
  return (
    isRecord(value) &&
    Object.keys(value).length === 13 &&
    Object.hasOwn(value, 'canvasEnabled') &&
    typeof value['canvasEnabled'] === 'boolean' &&
    Object.hasOwn(value, 'masonryEnabled') &&
    typeof value['masonryEnabled'] === 'boolean' &&
    Object.hasOwn(value, 'editorWidth') &&
    typeof value['editorWidth'] === 'number' &&
    Number.isSafeInteger(value['editorWidth']) &&
    value['editorWidth'] >= 0 &&
    value['editorWidth'] <= 16384 &&
    Object.hasOwn(value, 'route') &&
    (value['route'] === 'input' || value['route'] === 'output') &&
    Object.hasOwn(value, 'follow') &&
    (value['follow'] === 'auto' || value['follow'] === 'manual') &&
    Object.hasOwn(value, 'mode') &&
    (value['mode'] === 'image' || value['mode'] === 'overview') &&
    Object.hasOwn(value, 'linked') &&
    typeof value['linked'] === 'boolean' &&
    Object.hasOwn(value, 'selection') &&
    (isSessionSelection(value['selection']) || value['selection'] === null) &&
    Object.hasOwn(value, 'comparison') &&
    isSessionComparison(value['comparison']) &&
    Object.hasOwn(value, 'cameras') &&
    Array.isArray(value['cameras']) &&
    value['cameras'].length <= 8192 &&
    value['cameras'].every((item: unknown) => isSessionCamera(item)) &&
    Object.hasOwn(value, 'navigation') &&
    isSessionNavigation(value['navigation']) &&
    Object.hasOwn(value, 'sourcePreview') &&
    typeof value['sourcePreview'] === 'boolean' &&
    Object.hasOwn(value, 'sharedInspection') &&
    (isSessionInspection(value['sharedInspection']) || value['sharedInspection'] === null)
  );
}
/** Validate a bounded SessionGraphView payload before dispatch. */
export function isSessionGraphView(value: unknown): value is Contract.SessionGraphView {
  return (
    isRecord(value) &&
    Object.keys(value).length === 3 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] >= -1000000000 &&
    value['x'] <= 1000000000 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] >= -1000000000 &&
    value['y'] <= 1000000000 &&
    Object.hasOwn(value, 'scale') &&
    typeof value['scale'] === 'number' &&
    Number.isFinite(value['scale']) &&
    value['scale'] >= 0.000001 &&
    value['scale'] <= 1000000
  );
}
/** Validate a bounded SessionNavigation payload before dispatch. */
export function isSessionNavigation(value: unknown): value is Contract.SessionNavigation {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'sceneOverview') &&
    typeof value['sceneOverview'] === 'boolean' &&
    Object.hasOwn(value, 'sceneId') &&
    ((typeof value['sceneId'] === 'string' && value['sceneId'].length <= 36) ||
      value['sceneId'] === null) &&
    Object.hasOwn(value, 'sourceId') &&
    ((typeof value['sourceId'] === 'string' && value['sourceId'].length <= 36) ||
      value['sourceId'] === null) &&
    Object.hasOwn(value, 'batchIndex') &&
    typeof value['batchIndex'] === 'number' &&
    Number.isSafeInteger(value['batchIndex']) &&
    value['batchIndex'] >= 0 &&
    value['batchIndex'] <= 8191 &&
    Object.hasOwn(value, 'batchOverview') &&
    typeof value['batchOverview'] === 'boolean'
  );
}
/** Validate a bounded SessionInspection payload before dispatch. */
export function isSessionInspection(value: unknown): value is Contract.SessionInspection {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'mode') &&
    (value['mode'] === 'fit' || value['mode'] === 'native' || value['mode'] === 'custom') &&
    Object.hasOwn(value, 'centerX') &&
    typeof value['centerX'] === 'number' &&
    Number.isFinite(value['centerX']) &&
    value['centerX'] >= -1000000 &&
    value['centerX'] <= 1000000 &&
    Object.hasOwn(value, 'centerY') &&
    typeof value['centerY'] === 'number' &&
    Number.isFinite(value['centerY']) &&
    value['centerY'] >= -1000000 &&
    value['centerY'] <= 1000000 &&
    Object.hasOwn(value, 'spanX') &&
    typeof value['spanX'] === 'number' &&
    Number.isFinite(value['spanX']) &&
    value['spanX'] >= 1e-12 &&
    value['spanX'] <= 1000000000000 &&
    Object.hasOwn(value, 'spanY') &&
    typeof value['spanY'] === 'number' &&
    Number.isFinite(value['spanY']) &&
    value['spanY'] >= 1e-12 &&
    value['spanY'] <= 1000000000000
  );
}
