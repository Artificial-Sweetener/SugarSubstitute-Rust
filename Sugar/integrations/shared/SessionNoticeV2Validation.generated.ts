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
/** Generated protocol validation; edit the authoritative session-notice-v2.schema.json. */
import type * as Contract from '../contracts/SessionNoticeV2Protocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded SessionNoticeTone payload before dispatch. */
export function isSessionNoticeTone(value: unknown): value is Contract.SessionNoticeTone {
  return value === 'info' || value === 'warn' || value === 'error';
}
/** Validate a bounded SessionNoticeNotSavedMessage payload before dispatch. */
export function isSessionNoticeNotSavedMessage(
  value: unknown,
): value is Contract.SessionNoticeNotSavedMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.session-notice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'not-saved' &&
    Object.hasOwn(value, 'notice') &&
    typeof value['notice'] === 'number' &&
    Number.isSafeInteger(value['notice']) &&
    value['notice'] >= 1 &&
    value['notice'] <= 9007199254740991 &&
    Object.hasOwn(value, 'workspace') &&
    typeof value['workspace'] === 'boolean' &&
    Object.hasOwn(value, 'inputs') &&
    typeof value['inputs'] === 'boolean'
  );
}
/** Validate a bounded SessionNoticeLineMessage payload before dispatch. */
export function isSessionNoticeLineMessage(
  value: unknown,
): value is Contract.SessionNoticeLineMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 8 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.session-notice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'line' &&
    Object.hasOwn(value, 'notice') &&
    typeof value['notice'] === 'number' &&
    Number.isSafeInteger(value['notice']) &&
    value['notice'] >= 1 &&
    value['notice'] <= 9007199254740991 &&
    Object.hasOwn(value, 'tone') &&
    isSessionNoticeTone(value['tone']) &&
    Object.hasOwn(value, 'text') &&
    typeof value['text'] === 'string' &&
    value['text'].length <= 800 &&
    Array.from(value['text']).length <= 400 &&
    new RegExp('^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$').test(
      value['text'],
    ) &&
    Object.hasOwn(value, 'lasting') &&
    typeof value['lasting'] === 'boolean'
  );
}
/** Validate a bounded SessionNoticeNoneMessage payload before dispatch. */
export function isSessionNoticeNoneMessage(
  value: unknown,
): value is Contract.SessionNoticeNoneMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.session-notice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'none'
  );
}
/** Validate a bounded SessionNoticeWatchMessage payload before dispatch. */
export function isSessionNoticeWatchMessage(
  value: unknown,
): value is Contract.SessionNoticeWatchMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.session-notice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'watch'
  );
}
/** Validate a bounded SessionNoticeShownMessage payload before dispatch. */
export function isSessionNoticeShownMessage(
  value: unknown,
): value is Contract.SessionNoticeShownMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.session-notice' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 2 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'shown' &&
    Object.hasOwn(value, 'notice') &&
    typeof value['notice'] === 'number' &&
    Number.isSafeInteger(value['notice']) &&
    value['notice'] >= 1 &&
    value['notice'] <= 9007199254740991
  );
}
/** Validate a bounded SessionNoticeHostMessage payload before dispatch. */
export function isSessionNoticeHostMessage(
  value: unknown,
): value is Contract.SessionNoticeHostMessage {
  return (
    isSessionNoticeNotSavedMessage(value) ||
    isSessionNoticeLineMessage(value) ||
    isSessionNoticeNoneMessage(value)
  );
}
/** Validate a bounded SessionNoticeClientMessage payload before dispatch. */
export function isSessionNoticeClientMessage(
  value: unknown,
): value is Contract.SessionNoticeClientMessage {
  return isSessionNoticeWatchMessage(value) || isSessionNoticeShownMessage(value);
}
/** Validate a bounded SessionNoticeMessage payload before dispatch. */
export function isSessionNoticeMessage(value: unknown): value is Contract.SessionNoticeMessage {
  return (
    isSessionNoticeNotSavedMessage(value) ||
    isSessionNoticeLineMessage(value) ||
    isSessionNoticeNoneMessage(value) ||
    isSessionNoticeWatchMessage(value) ||
    isSessionNoticeShownMessage(value)
  );
}
