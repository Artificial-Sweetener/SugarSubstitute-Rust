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
/** Encode and validate the SugarCubes memory budget protocol, version one. */
import { isRecord } from '../../types/common.js';
import {
  DEFAULT_MEMORY_ALLOWANCE_BYTES,
  type MemoryPressureLevel,
  type MemoryUsage,
} from './MemoryBudgetRegistry.js';

/** Protocol identifier shared with desktop hosts. */
export const MEMORY_PROTOCOL = 'sugarcubes.memory';
/** Protocol version; it advances independently of the panel transport. */
export const MEMORY_PROTOCOL_VERSION = 1;
const MAX_CONSUMERS = 16;
const CONSUMER_ID = /^[a-z0-9.-]{1,48}$/;
const LEVELS: readonly MemoryPressureLevel[] = ['normal', 'elevated', 'high', 'critical'];

/** The user's application memory ceiling. */
export type MemoryPreference =
  | { readonly mode: 'share'; readonly percent: number }
  | { readonly mode: 'fixed'; readonly bytes: number };

/** Page-direction usage report. */
export interface MemoryUsageMessage {
  readonly protocol: typeof MEMORY_PROTOCOL;
  readonly version: typeof MEMORY_PROTOCOL_VERSION;
  readonly session: string;
  readonly type: 'usage';
  readonly usedBytes: number;
  readonly demandBytes: number;
  readonly consumers: readonly { readonly id: string; readonly bytes: number }[];
}

/** Page-direction stored preference. */
export interface MemoryPreferenceMessage {
  readonly protocol: typeof MEMORY_PROTOCOL;
  readonly version: typeof MEMORY_PROTOCOL_VERSION;
  readonly session: string;
  readonly type: 'preference';
  readonly preference: MemoryPreference;
}

/** Every page-direction memory message. */
export type MemoryClientMessage = MemoryUsageMessage | MemoryPreferenceMessage;

/** Host-direction allowance for the page's caches. */
export interface MemoryAllowance {
  readonly generation: number;
  readonly level: MemoryPressureLevel;
  readonly allowanceBytes: number;
  readonly ceilingBytes: number;
  readonly totalBytes: number;
  readonly preference: MemoryPreference;
}

/**
 * Report usage with room to grow: half again what the caches hold, and never less room than the
 * standalone allowance, so joining a host never shrinks a page's caches below it.
 */
export function usageMessage(session: string, usage: MemoryUsage): MemoryUsageMessage {
  const used = Math.max(0, Math.round(usage.usedBytes));
  const consumers = usage.consumers
    .filter((consumer) => CONSUMER_ID.test(consumer.id))
    .slice(0, MAX_CONSUMERS)
    .map((consumer) => ({ id: consumer.id, bytes: Math.max(0, Math.round(consumer.bytes)) }));
  return {
    protocol: MEMORY_PROTOCOL,
    version: MEMORY_PROTOCOL_VERSION,
    session,
    type: 'usage',
    usedBytes: used,
    demandBytes: used + Math.max(DEFAULT_MEMORY_ALLOWANCE_BYTES, Math.round(used / 2)),
    consumers,
  };
}

/** Report the stored preference. */
export function preferenceMessage(
  session: string,
  preference: MemoryPreference,
): MemoryPreferenceMessage {
  return {
    protocol: MEMORY_PROTOCOL,
    version: MEMORY_PROTOCOL_VERSION,
    session,
    type: 'preference',
    preference,
  };
}

/** Accept only a well-formed allowance addressed to `session`. */
export function readAllowance(value: unknown, session: string): MemoryAllowance | null {
  if (
    !isRecord(value) ||
    value.protocol !== MEMORY_PROTOCOL ||
    value.version !== MEMORY_PROTOCOL_VERSION ||
    value.session !== session ||
    value.type !== 'allowance'
  )
    return null;
  const level = LEVELS.find((candidate) => candidate === value.level);
  const preference = readPreference(value.preference);
  const counts = [value.generation, value.allowanceBytes, value.ceilingBytes, value.totalBytes];
  if (!level || !preference || !counts.every(isByteCount)) return null;
  return {
    generation: value.generation as number,
    level,
    allowanceBytes: value.allowanceBytes as number,
    ceilingBytes: value.ceilingBytes as number,
    totalBytes: value.totalBytes as number,
    preference,
  };
}

function readPreference(value: unknown): MemoryPreference | null {
  if (!isRecord(value)) return null;
  if (value.mode === 'share' && Number.isInteger(value.percent))
    return { mode: 'share', percent: value.percent as number };
  if (value.mode === 'fixed' && isByteCount(value.bytes))
    return { mode: 'fixed', bytes: value.bytes as number };
  return null;
}

function isByteCount(value: unknown): boolean {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}
