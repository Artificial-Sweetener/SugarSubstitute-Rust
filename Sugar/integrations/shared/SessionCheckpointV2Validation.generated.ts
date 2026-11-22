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
/** Generated protocol validation; edit the authoritative session-checkpoint-v2.schema.json. */
import type * as Contract from '../contracts/SessionCheckpointV2Protocol.generated.js';
import {
  isSessionScope,
  isSessionSource,
  isSessionAsset,
  isSessionOutput,
  isSessionSelection,
  isSessionCamera,
  isSessionComparison,
  isSessionDocument,
  isSessionInput,
  isSessionSubmission,
  isSessionPlacement,
  isSessionNavigation,
  isSessionInspection,
} from './SessionValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded SessionScroll payload before dispatch. */
export function isSessionScroll(value: unknown): value is Contract.SessionScroll {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'cube') &&
    ((typeof value['cube'] === 'string' && value['cube'].length <= 256) ||
      value['cube'] === null) &&
    Object.hasOwn(value, 'offset') &&
    typeof value['offset'] === 'number' &&
    Number.isSafeInteger(value['offset']) &&
    value['offset'] >= 0 &&
    value['offset'] <= 10000000
  );
}
/** Validate a bounded SessionView payload before dispatch. */
export function isSessionView(value: unknown): value is Contract.SessionView {
  return (
    isRecord(value) &&
    Object.keys(value).length === 14 &&
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
    (isSessionInspection(value['sharedInspection']) || value['sharedInspection'] === null) &&
    Object.hasOwn(value, 'scroll') &&
    (isSessionScroll(value['scroll']) || value['scroll'] === null)
  );
}
/** Validate a bounded SessionEntry payload before dispatch. */
export function isSessionEntry(value: unknown): value is Contract.SessionEntry {
  return (
    isRecord(value) &&
    Object.keys(value).length === 9 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 36 &&
    Object.hasOwn(value, 'document') &&
    isSessionDocument(value['document']) &&
    Object.hasOwn(value, 'input') &&
    (isSessionInput(value['input']) || value['input'] === null) &&
    Object.hasOwn(value, 'view') &&
    isSessionView(value['view']) &&
    Object.hasOwn(value, 'sourceRevision') &&
    typeof value['sourceRevision'] === 'number' &&
    Number.isSafeInteger(value['sourceRevision']) &&
    value['sourceRevision'] >= 0 &&
    value['sourceRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'navigationRevision') &&
    typeof value['navigationRevision'] === 'number' &&
    Number.isSafeInteger(value['navigationRevision']) &&
    value['navigationRevision'] >= 0 &&
    value['navigationRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'sources') &&
    Array.isArray(value['sources']) &&
    value['sources'].length <= 8192 &&
    value['sources'].every((item: unknown) => isSessionSource(item)) &&
    Object.hasOwn(value, 'outputs') &&
    Array.isArray(value['outputs']) &&
    value['outputs'].length <= 8192 &&
    value['outputs'].every((item: unknown) => isSessionOutput(item)) &&
    Object.hasOwn(value, 'submissions') &&
    Array.isArray(value['submissions']) &&
    value['submissions'].length <= 8192 &&
    value['submissions'].every((item: unknown) => isSessionSubmission(item))
  );
}
/** Validate a bounded SessionCheckpoint payload before dispatch. */
export function isSessionCheckpoint(value: unknown): value is Contract.SessionCheckpoint {
  return (
    isRecord(value) &&
    Object.keys(value).length === 14 &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 36 &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 0 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'previousId') &&
    ((typeof value['previousId'] === 'string' && value['previousId'].length <= 36) ||
      value['previousId'] === null) &&
    Object.hasOwn(value, 'capturedAt') &&
    typeof value['capturedAt'] === 'number' &&
    Number.isSafeInteger(value['capturedAt']) &&
    value['capturedAt'] >= 0 &&
    value['capturedAt'] <= 9007199254740991 &&
    Object.hasOwn(value, 'scope') &&
    isSessionScope(value['scope']) &&
    Object.hasOwn(value, 'producer') &&
    typeof value['producer'] === 'string' &&
    value['producer'].length <= 256 &&
    Object.hasOwn(value, 'frontend') &&
    typeof value['frontend'] === 'string' &&
    value['frontend'].length <= 256 &&
    Object.hasOwn(value, 'sugarcubes') &&
    typeof value['sugarcubes'] === 'string' &&
    value['sugarcubes'].length <= 256 &&
    Object.hasOwn(value, 'entries') &&
    Array.isArray(value['entries']) &&
    value['entries'].length <= 256 &&
    value['entries'].every((item: unknown) => isSessionEntry(item)) &&
    Object.hasOwn(value, 'activeEntry') &&
    ((typeof value['activeEntry'] === 'string' && value['activeEntry'].length <= 36) ||
      value['activeEntry'] === null) &&
    Object.hasOwn(value, 'route') &&
    (value['route'] === 'workflow' || value['route'] === 'settings') &&
    Object.hasOwn(value, 'placement') &&
    (isSessionPlacement(value['placement']) || value['placement'] === null) &&
    Object.hasOwn(value, 'assets') &&
    Array.isArray(value['assets']) &&
    value['assets'].length <= 65536 &&
    value['assets'].every((item: unknown) => isSessionAsset(item))
  );
}
/** Validate a bounded SessionCheckpointValue payload before dispatch. */
export function isSessionCheckpointValue(value: unknown): value is Contract.SessionCheckpointValue {
  return isSessionCheckpoint(value);
}
