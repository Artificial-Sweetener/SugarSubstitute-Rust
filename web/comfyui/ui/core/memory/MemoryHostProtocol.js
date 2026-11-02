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
import { DEFAULT_MEMORY_ALLOWANCE_BYTES, } from './MemoryBudgetRegistry.js';
/** Protocol identifier shared with desktop hosts. */
export const MEMORY_PROTOCOL = 'sugarcubes.memory';
/** Protocol version; it advances independently of the panel transport. */
export const MEMORY_PROTOCOL_VERSION = 1;
const MAX_CONSUMERS = 16;
const CONSUMER_ID = /^[a-z0-9.-]{1,48}$/;
const LEVELS = ['normal', 'elevated', 'high', 'critical'];
/**
 * Report usage with room to grow: half again what the caches hold, and never less room than the
 * standalone allowance, so joining a host never shrinks a page's caches below it.
 */
export function usageMessage(session, usage) {
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
export function preferenceMessage(session, preference) {
    return {
        protocol: MEMORY_PROTOCOL,
        version: MEMORY_PROTOCOL_VERSION,
        session,
        type: 'preference',
        preference,
    };
}
/** Accept only a well-formed allowance addressed to `session`. */
export function readAllowance(value, session) {
    if (!isRecord(value) ||
        value.protocol !== MEMORY_PROTOCOL ||
        value.version !== MEMORY_PROTOCOL_VERSION ||
        value.session !== session ||
        value.type !== 'allowance')
        return null;
    const level = LEVELS.find((candidate) => candidate === value.level);
    const preference = readPreference(value.preference);
    const counts = [value.generation, value.allowanceBytes, value.ceilingBytes, value.totalBytes];
    if (!level || !preference || !counts.every(isByteCount))
        return null;
    return {
        generation: value.generation,
        level,
        allowanceBytes: value.allowanceBytes,
        ceilingBytes: value.ceilingBytes,
        totalBytes: value.totalBytes,
        preference,
    };
}
function readPreference(value) {
    if (!isRecord(value))
        return null;
    if (value.mode === 'share' && Number.isInteger(value.percent))
        return { mode: 'share', percent: value.percent };
    if (value.mode === 'fixed' && isByteCount(value.bytes))
        return { mode: 'fixed', bytes: value.bytes };
    return null;
}
function isByteCount(value) {
    return Number.isSafeInteger(value) && value >= 0;
}
