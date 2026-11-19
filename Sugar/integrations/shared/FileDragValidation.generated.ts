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
/** Generated protocol validation; edit the authoritative file-drag.schema.json. */
import type * as Contract from '../contracts/FileDragProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded FileDragFile payload before dispatch. */
export function isFileDragFile(value: unknown): value is Contract.FileDragFile {
  return (
    isRecord(value) &&
    Object.keys(value).length === 1 &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 255
  );
}
/** Validate a bounded FileDragEnteredMessage payload before dispatch. */
export function isFileDragEnteredMessage(value: unknown): value is Contract.FileDragEnteredMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-drag' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'entered' &&
    Object.hasOwn(value, 'files') &&
    Array.isArray(value['files']) &&
    value['files'].length >= 1 &&
    value['files'].length <= 16 &&
    value['files'].every((item: unknown) => isFileDragFile(item))
  );
}
/** Validate a bounded FileDragLeftMessage payload before dispatch. */
export function isFileDragLeftMessage(value: unknown): value is Contract.FileDragLeftMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-drag' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'left'
  );
}
/** Validate a bounded FileDragHostMessage payload before dispatch. */
export function isFileDragHostMessage(value: unknown): value is Contract.FileDragHostMessage {
  return isFileDragEnteredMessage(value) || isFileDragLeftMessage(value);
}
/** Validate a bounded FileDragMessage payload before dispatch. */
export function isFileDragMessage(value: unknown): value is Contract.FileDragMessage {
  return isFileDragEnteredMessage(value) || isFileDragLeftMessage(value);
}
