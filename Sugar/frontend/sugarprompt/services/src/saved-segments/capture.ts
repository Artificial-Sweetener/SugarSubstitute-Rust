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
/** Capture only closed plain data records, without executing property getters. Reflection failures retain their original cause. */
import { strip } from '../../../core/src/prompt/text.js';

/** Capture plain saved-segment data without executing untrusted property getters. */
export function captureRecord(
  value: unknown,
  keys: readonly string[],
): Readonly<Record<string, unknown>> {
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError('invalid-saved-segment-record');
  const prototype: unknown = Object.getPrototypeOf(value);
  if (prototype !== null && prototype !== Object.prototype)
    throw new TypeError('invalid-saved-segment-record');
  const names = Reflect.ownKeys(value);
  if (
    names.length !== keys.length ||
    names.some((key) => typeof key !== 'string' || !keys.includes(key))
  )
    throw new TypeError('invalid-saved-segment-record');
  const result: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !('value' in descriptor))
      throw new TypeError('invalid-saved-segment-record');
    result[key] = descriptor.value as unknown;
  }
  return Object.freeze(result);
}

/** Copy bounded dense ordinary arrays through data descriptors, never foreign iteration or indexed getters. */
export function captureArray(value: unknown, limit: number): readonly unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype)
    throw new TypeError('invalid-saved-segment-array');
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, 'length');
  const length: unknown = lengthDescriptor?.value;
  if (typeof length !== 'number' || !Number.isSafeInteger(length) || length < 0 || length > limit)
    throw new TypeError('invalid-saved-segment-array');
  if (Reflect.ownKeys(value).length !== length + 1)
    throw new TypeError('invalid-saved-segment-array');
  const result: unknown[] = [];
  for (let index = 0; index < length; index++) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor || !('value' in descriptor)) throw new TypeError('invalid-saved-segment-array');
    result.push(descriptor.value as unknown);
  }
  return Object.freeze(result);
}

/** Bound work before Python-compatible eligibility checks; authored bytes remain unchanged. */
export function captureText(value: unknown, limit: number): string {
  if (typeof value !== 'string' || value.length > limit || !strip(value))
    throw new TypeError('invalid-saved-segment-text');
  return value;
}

/** Capture a closed status before inspecting a read payload. Status accessors never run. */
export function captureReply(
  value: unknown,
  allowed: readonly string[],
): Readonly<Record<string, unknown>> {
  if (value === null || typeof value !== 'object')
    throw new TypeError('invalid-saved-segment-reply');
  const descriptor = Object.getOwnPropertyDescriptor(value, 'status');
  const status: unknown = descriptor && 'value' in descriptor ? descriptor.value : undefined;
  if (typeof status !== 'string' || !allowed.includes(status))
    throw new TypeError('invalid-saved-segment-reply');
  const result = captureRecord(value, status === 'ready' ? ['status', 'segments'] : ['status']);
  if (result.status !== status) throw new TypeError('invalid-saved-segment-reply');
  return result;
}
