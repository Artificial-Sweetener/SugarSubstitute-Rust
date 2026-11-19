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
/** Generated protocol validation; edit the authoritative update-report.schema.json. */
import type * as Contract from '../contracts/UpdateReportProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded UpdateReportStateMessage payload before dispatch. */
export function isUpdateReportStateMessage(
  value: unknown,
): value is Contract.UpdateReportStateMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.update-report' &&
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
    value['revision'] >= 0 &&
    value['revision'] <= 9007199254740991 &&
    Object.hasOwn(value, 'available') &&
    typeof value['available'] === 'boolean'
  );
}
/** Validate a bounded UpdateReportWatchMessage payload before dispatch. */
export function isUpdateReportWatchMessage(
  value: unknown,
): value is Contract.UpdateReportWatchMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 4 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.update-report' &&
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
/** Validate a bounded UpdateReportOpenMessage payload before dispatch. */
export function isUpdateReportOpenMessage(
  value: unknown,
): value is Contract.UpdateReportOpenMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.update-report' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'open' &&
    Object.hasOwn(value, 'revision') &&
    typeof value['revision'] === 'number' &&
    Number.isSafeInteger(value['revision']) &&
    value['revision'] >= 0 &&
    value['revision'] <= 9007199254740991
  );
}
/** Validate a bounded UpdateReportClientMessage payload before dispatch. */
export function isUpdateReportClientMessage(
  value: unknown,
): value is Contract.UpdateReportClientMessage {
  return isUpdateReportWatchMessage(value) || isUpdateReportOpenMessage(value);
}
/** Validate a bounded UpdateReportMessage payload before dispatch. */
export function isUpdateReportMessage(value: unknown): value is Contract.UpdateReportMessage {
  return (
    isUpdateReportStateMessage(value) ||
    isUpdateReportWatchMessage(value) ||
    isUpdateReportOpenMessage(value)
  );
}
