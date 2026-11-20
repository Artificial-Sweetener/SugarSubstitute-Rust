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
/** Generated protocol validation; edit the authoritative file-choice.schema.json. */
import type * as Contract from '../contracts/FileChoiceProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded FileChoiceFailure payload before dispatch. */
export function isFileChoiceFailure(value: unknown): value is Contract.FileChoiceFailure {
  return value === 'busy' || value === 'unavailable';
}
/** Validate a bounded FileChoiceChosenMessage payload before dispatch. */
export function isFileChoiceChosenMessage(
  value: unknown,
): value is Contract.FileChoiceChosenMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-choice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'chosen' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 255 &&
    Object.hasOwn(value, 'relative') &&
    typeof value['relative'] === 'string' &&
    value['relative'].length <= 1024
  );
}
/** Validate a bounded FileChoiceDismissedMessage payload before dispatch. */
export function isFileChoiceDismissedMessage(
  value: unknown,
): value is Contract.FileChoiceDismissedMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-choice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'dismissed' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request'])
  );
}
/** Validate a bounded FileChoiceFailedMessage payload before dispatch. */
export function isFileChoiceFailedMessage(
  value: unknown,
): value is Contract.FileChoiceFailedMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-choice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'failed' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'reason') &&
    isFileChoiceFailure(value['reason'])
  );
}
/** Validate a bounded FileChoiceOpenMessage payload before dispatch. */
export function isFileChoiceOpenMessage(value: unknown): value is Contract.FileChoiceOpenMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 8 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-choice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'open' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'folder') &&
    typeof value['folder'] === 'string' &&
    value['folder'].length <= 1024 &&
    Object.hasOwn(value, 'start') &&
    typeof value['start'] === 'string' &&
    value['start'].length <= 1024 &&
    Object.hasOwn(value, 'extension') &&
    typeof value['extension'] === 'string' &&
    value['extension'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['extension'])
  );
}
/** Validate a bounded FileChoiceSaveMessage payload before dispatch. */
export function isFileChoiceSaveMessage(value: unknown): value is Contract.FileChoiceSaveMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 9 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.file-choice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'save' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'folder') &&
    typeof value['folder'] === 'string' &&
    value['folder'].length <= 1024 &&
    Object.hasOwn(value, 'start') &&
    typeof value['start'] === 'string' &&
    value['start'].length <= 1024 &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 255 &&
    Object.hasOwn(value, 'extension') &&
    typeof value['extension'] === 'string' &&
    value['extension'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['extension'])
  );
}
/** Validate a bounded FileChoiceHostMessage payload before dispatch. */
export function isFileChoiceHostMessage(value: unknown): value is Contract.FileChoiceHostMessage {
  return (
    isFileChoiceChosenMessage(value) ||
    isFileChoiceDismissedMessage(value) ||
    isFileChoiceFailedMessage(value)
  );
}
/** Validate a bounded FileChoiceClientMessage payload before dispatch. */
export function isFileChoiceClientMessage(
  value: unknown,
): value is Contract.FileChoiceClientMessage {
  return isFileChoiceOpenMessage(value) || isFileChoiceSaveMessage(value);
}
/** Validate a bounded FileChoiceMessage payload before dispatch. */
export function isFileChoiceMessage(value: unknown): value is Contract.FileChoiceMessage {
  return (
    isFileChoiceChosenMessage(value) ||
    isFileChoiceDismissedMessage(value) ||
    isFileChoiceFailedMessage(value) ||
    isFileChoiceOpenMessage(value) ||
    isFileChoiceSaveMessage(value)
  );
}
