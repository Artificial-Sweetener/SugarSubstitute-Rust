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
/** Generated protocol validation; edit the authoritative release-notes.schema.json. */
import type * as Contract from '../contracts/ReleaseNotesProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded ReleaseNotesFailure payload before dispatch. */
export function isReleaseNotesFailure(value: unknown): value is Contract.ReleaseNotesFailure {
  return value === 'unavailable' || value === 'not-published';
}
/** Validate a bounded ReleaseNotesRelease payload before dispatch. */
export function isReleaseNotesRelease(value: unknown): value is Contract.ReleaseNotesRelease {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'version') &&
    typeof value['version'] === 'string' &&
    value['version'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['version']) &&
    Object.hasOwn(value, 'notes') &&
    typeof value['notes'] === 'string' &&
    value['notes'].length <= 262144
  );
}
/** Validate a bounded ReleaseNotesStateMessage payload before dispatch. */
export function isReleaseNotesStateMessage(
  value: unknown,
): value is Contract.ReleaseNotesStateMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.release-notes' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'state' &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 1 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'product') &&
    typeof value['product'] === 'string' &&
    value['product'].length <= 64 &&
    Object.hasOwn(value, 'currentVersion') &&
    typeof value['currentVersion'] === 'string' &&
    value['currentVersion'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['currentVersion'])
  );
}
/** Validate a bounded ReleaseNotesOpenMessage payload before dispatch. */
export function isReleaseNotesOpenMessage(
  value: unknown,
): value is Contract.ReleaseNotesOpenMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.release-notes' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'open'
  );
}
/** Validate a bounded ReleaseNotesNotesMessage payload before dispatch. */
export function isReleaseNotesNotesMessage(
  value: unknown,
): value is Contract.ReleaseNotesNotesMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.release-notes' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'notes' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'releases') &&
    Array.isArray(value['releases']) &&
    value['releases'].length >= 1 &&
    value['releases'].length <= 64 &&
    value['releases'].every((item: unknown) => isReleaseNotesRelease(item)) &&
    Object.hasOwn(value, 'omitted') &&
    typeof value['omitted'] === 'number' &&
    Number.isSafeInteger(value['omitted']) &&
    value['omitted'] >= 0 &&
    value['omitted'] <= 9007199254740991
  );
}
/** Validate a bounded ReleaseNotesFailedMessage payload before dispatch. */
export function isReleaseNotesFailedMessage(
  value: unknown,
): value is Contract.ReleaseNotesFailedMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.release-notes' &&
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
    isReleaseNotesFailure(value['reason'])
  );
}
/** Validate a bounded ReleaseNotesWatchMessage payload before dispatch. */
export function isReleaseNotesWatchMessage(
  value: unknown,
): value is Contract.ReleaseNotesWatchMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.release-notes' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'watch'
  );
}
/** Validate a bounded ReleaseNotesLoadMessage payload before dispatch. */
export function isReleaseNotesLoadMessage(
  value: unknown,
): value is Contract.ReleaseNotesLoadMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.release-notes' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'load' &&
    Object.hasOwn(value, 'request') &&
    typeof value['request'] === 'string' &&
    value['request'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['request']) &&
    Object.hasOwn(value, 'since') &&
    typeof value['since'] === 'string' &&
    value['since'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['since'])
  );
}
/** Validate a bounded ReleaseNotesHostMessage payload before dispatch. */
export function isReleaseNotesHostMessage(
  value: unknown,
): value is Contract.ReleaseNotesHostMessage {
  return (
    isReleaseNotesStateMessage(value) ||
    isReleaseNotesOpenMessage(value) ||
    isReleaseNotesNotesMessage(value) ||
    isReleaseNotesFailedMessage(value)
  );
}
/** Validate a bounded ReleaseNotesClientMessage payload before dispatch. */
export function isReleaseNotesClientMessage(
  value: unknown,
): value is Contract.ReleaseNotesClientMessage {
  return isReleaseNotesWatchMessage(value) || isReleaseNotesLoadMessage(value);
}
/** Validate a bounded ReleaseNotesMessage payload before dispatch. */
export function isReleaseNotesMessage(value: unknown): value is Contract.ReleaseNotesMessage {
  return (
    isReleaseNotesStateMessage(value) ||
    isReleaseNotesOpenMessage(value) ||
    isReleaseNotesNotesMessage(value) ||
    isReleaseNotesFailedMessage(value) ||
    isReleaseNotesWatchMessage(value) ||
    isReleaseNotesLoadMessage(value)
  );
}
