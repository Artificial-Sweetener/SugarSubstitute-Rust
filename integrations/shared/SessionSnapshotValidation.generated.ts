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
import { isSessionGraphView, isSessionView } from './SessionViewValidation.generated.js';
import {
  isSessionSource,
  isSessionOutput,
  isSessionSubmission,
  isSessionScope,
  isSessionAsset,
} from './SessionOutputValidation.generated.js';
import { isRecord } from './ContractRecord.js';
/** Validate a bounded SessionDocument payload before dispatch. */
export function isSessionDocument(value: unknown): value is Contract.SessionDocument {
  return (
    isRecord(value) &&
    Object.keys(value).length === 9 &&
    Object.hasOwn(value, 'graphJson') &&
    typeof value['graphJson'] === 'string' &&
    value['graphJson'].length <= 33554432 &&
    Object.hasOwn(value, 'graphRevision') &&
    typeof value['graphRevision'] === 'number' &&
    Number.isSafeInteger(value['graphRevision']) &&
    value['graphRevision'] >= 0 &&
    value['graphRevision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'path') &&
    ((typeof value['path'] === 'string' && value['path'].length <= 4096) ||
      value['path'] === null) &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 4096 &&
    Object.hasOwn(value, 'temporary') &&
    typeof value['temporary'] === 'boolean' &&
    Object.hasOwn(value, 'dirty') &&
    typeof value['dirty'] === 'boolean' &&
    Object.hasOwn(value, 'sourceIdentity') &&
    ((typeof value['sourceIdentity'] === 'string' && value['sourceIdentity'].length <= 4096) ||
      value['sourceIdentity'] === null) &&
    Object.hasOwn(value, 'view') &&
    isSessionGraphView(value['view']) &&
    Object.hasOwn(value, 'baselineGraphJson') &&
    ((typeof value['baselineGraphJson'] === 'string' &&
      value['baselineGraphJson'].length <= 33554432) ||
      value['baselineGraphJson'] === null)
  );
}
/** Validate a bounded SessionInput payload before dispatch. */
export function isSessionInput(value: unknown): value is Contract.SessionInput {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'documentId') &&
    typeof value['documentId'] === 'string' &&
    value['documentId'].length <= 256 &&
    Object.hasOwn(value, 'generation') &&
    typeof value['generation'] === 'number' &&
    Number.isSafeInteger(value['generation']) &&
    value['generation'] >= 0 &&
    value['generation'] <= 9007199254740991 &&
    Object.hasOwn(value, 'signature') &&
    typeof value['signature'] === 'string' &&
    value['signature'].length <= 64 &&
    Object.hasOwn(value, 'surface') &&
    ((typeof value['surface'] === 'string' && value['surface'].length <= 256) ||
      value['surface'] === null) &&
    Object.hasOwn(value, 'mask') &&
    ((typeof value['mask'] === 'string' && value['mask'].length <= 256) || value['mask'] === null)
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
/** Validate a bounded SessionRect payload before dispatch. */
export function isSessionRect(value: unknown): value is Contract.SessionRect {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'x') &&
    typeof value['x'] === 'number' &&
    Number.isFinite(value['x']) &&
    value['x'] >= -1000000 &&
    value['x'] <= 1000000 &&
    Object.hasOwn(value, 'y') &&
    typeof value['y'] === 'number' &&
    Number.isFinite(value['y']) &&
    value['y'] >= -1000000 &&
    value['y'] <= 1000000 &&
    Object.hasOwn(value, 'width') &&
    typeof value['width'] === 'number' &&
    Number.isFinite(value['width']) &&
    value['width'] >= 1 &&
    value['width'] <= 1000000 &&
    Object.hasOwn(value, 'height') &&
    typeof value['height'] === 'number' &&
    Number.isFinite(value['height']) &&
    value['height'] >= 1 &&
    value['height'] <= 1000000
  );
}
/** Validate a bounded SessionPlacement payload before dispatch. */
export function isSessionPlacement(value: unknown): value is Contract.SessionPlacement {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'bounds') &&
    isSessionRect(value['bounds']) &&
    Object.hasOwn(value, 'maximized') &&
    typeof value['maximized'] === 'boolean' &&
    Object.hasOwn(value, 'monitor') &&
    typeof value['monitor'] === 'string' &&
    value['monitor'].length <= 256 &&
    Object.hasOwn(value, 'workArea') &&
    isSessionRect(value['workArea']) &&
    Object.hasOwn(value, 'dpi') &&
    typeof value['dpi'] === 'number' &&
    Number.isFinite(value['dpi']) &&
    value['dpi'] >= 24 &&
    value['dpi'] <= 960
  );
}
/** Validate a bounded SessionCheckpoint payload before dispatch. */
export function isSessionCheckpoint(value: unknown): value is Contract.SessionCheckpoint {
  return (
    isRecord(value) &&
    Object.keys(value).length === 14 &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
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
