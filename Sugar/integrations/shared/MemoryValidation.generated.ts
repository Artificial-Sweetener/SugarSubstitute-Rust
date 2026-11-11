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
/** Generated protocol validation; edit the authoritative memory.schema.json. */
import type * as Contract from '../contracts/MemoryProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded MemoryLevel payload before dispatch. */
export function isMemoryLevel(value: unknown): value is Contract.MemoryLevel {
  return value === 'normal' || value === 'elevated' || value === 'high' || value === 'critical';
}
/** Validate a bounded MemoryPreference payload before dispatch. */
export function isMemoryPreference(value: unknown): value is Contract.MemoryPreference {
  return (
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'mode') &&
      value['mode'] === 'share' &&
      Object.hasOwn(value, 'percent') &&
      typeof value['percent'] === 'number' &&
      Number.isSafeInteger(value['percent']) &&
      value['percent'] >= 10 &&
      value['percent'] <= 75) ||
    (isRecord(value) &&
      Object.keys(value).length === 2 &&
      Object.hasOwn(value, 'mode') &&
      value['mode'] === 'fixed' &&
      Object.hasOwn(value, 'bytes') &&
      typeof value['bytes'] === 'number' &&
      Number.isSafeInteger(value['bytes']) &&
      value['bytes'] >= 1 &&
      value['bytes'] <= 9007199254740991)
  );
}
/** Validate a bounded MemoryConsumer payload before dispatch. */
export function isMemoryConsumer(value: unknown): value is Contract.MemoryConsumer {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'id') &&
    typeof value['id'] === 'string' &&
    value['id'].length <= 48 &&
    new RegExp('^[a-z0-9.-]{1,48}$').test(value['id']) &&
    Object.hasOwn(value, 'bytes') &&
    typeof value['bytes'] === 'number' &&
    Number.isSafeInteger(value['bytes']) &&
    value['bytes'] >= 0 &&
    value['bytes'] <= 9007199254740991
  );
}
/** Validate a bounded MemoryUsageMessage payload before dispatch. */
export function isMemoryUsageMessage(value: unknown): value is Contract.MemoryUsageMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 7 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.memory' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'usage' &&
    Object.hasOwn(value, 'usedBytes') &&
    typeof value['usedBytes'] === 'number' &&
    Number.isSafeInteger(value['usedBytes']) &&
    value['usedBytes'] >= 0 &&
    value['usedBytes'] <= 9007199254740991 &&
    Object.hasOwn(value, 'demandBytes') &&
    typeof value['demandBytes'] === 'number' &&
    Number.isSafeInteger(value['demandBytes']) &&
    value['demandBytes'] >= 0 &&
    value['demandBytes'] <= 9007199254740991 &&
    Object.hasOwn(value, 'consumers') &&
    Array.isArray(value['consumers']) &&
    value['consumers'].length <= 16 &&
    value['consumers'].every((item: unknown) => isMemoryConsumer(item))
  );
}
/** Validate a bounded MemoryPreferenceMessage payload before dispatch. */
export function isMemoryPreferenceMessage(
  value: unknown,
): value is Contract.MemoryPreferenceMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 5 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.memory' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'preference' &&
    Object.hasOwn(value, 'preference') &&
    isMemoryPreference(value['preference'])
  );
}
/** Validate a bounded MemoryAllowanceMessage payload before dispatch. */
export function isMemoryAllowanceMessage(value: unknown): value is Contract.MemoryAllowanceMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 10 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.memory' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'allowance' &&
    Object.hasOwn(value, 'generation') &&
    typeof value['generation'] === 'number' &&
    Number.isSafeInteger(value['generation']) &&
    value['generation'] >= 0 &&
    value['generation'] <= 9007199254740991 &&
    Object.hasOwn(value, 'level') &&
    isMemoryLevel(value['level']) &&
    Object.hasOwn(value, 'allowanceBytes') &&
    typeof value['allowanceBytes'] === 'number' &&
    Number.isSafeInteger(value['allowanceBytes']) &&
    value['allowanceBytes'] >= 0 &&
    value['allowanceBytes'] <= 9007199254740991 &&
    Object.hasOwn(value, 'ceilingBytes') &&
    typeof value['ceilingBytes'] === 'number' &&
    Number.isSafeInteger(value['ceilingBytes']) &&
    value['ceilingBytes'] >= 0 &&
    value['ceilingBytes'] <= 9007199254740991 &&
    Object.hasOwn(value, 'totalBytes') &&
    typeof value['totalBytes'] === 'number' &&
    Number.isSafeInteger(value['totalBytes']) &&
    value['totalBytes'] >= 0 &&
    value['totalBytes'] <= 9007199254740991 &&
    Object.hasOwn(value, 'preference') &&
    isMemoryPreference(value['preference'])
  );
}
/** Validate a bounded MemoryClientMessage payload before dispatch. */
export function isMemoryClientMessage(value: unknown): value is Contract.MemoryClientMessage {
  return isMemoryUsageMessage(value) || isMemoryPreferenceMessage(value);
}
/** Validate a bounded MemoryMessage payload before dispatch. */
export function isMemoryMessage(value: unknown): value is Contract.MemoryMessage {
  return (
    isMemoryUsageMessage(value) ||
    isMemoryPreferenceMessage(value) ||
    isMemoryAllowanceMessage(value)
  );
}
