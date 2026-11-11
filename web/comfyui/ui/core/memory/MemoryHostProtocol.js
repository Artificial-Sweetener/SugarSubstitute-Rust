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
/**
 * Encode and admit the SugarCubes memory budget protocol, version one.
 *
 * `integrations/contracts/memory.schema.json` owns the wire shapes and bounds; this module only
 * derives page reports from the cache registry and admits host allowances through the generated
 * guard, bundled for the browser by `MemoryRuntime`, so the page and every host decode one schema.
 */
import { isMemoryAllowanceMessage, } from '../embedded/MemoryRuntime.js';
import { DEFAULT_MEMORY_ALLOWANCE_BYTES } from './MemoryBudgetRegistry.js';
/** Protocol identifier shared with desktop hosts. */
export const MEMORY_PROTOCOL = 'sugarcubes.memory';
/** Protocol version; it advances independently of the panel transport. */
export const MEMORY_PROTOCOL_VERSION = 1;
const MAX_CONSUMERS = 16;
const CONSUMER_ID = /^[a-z0-9.-]{1,48}$/;
/** Round a measured byte count onto the schema's exact, non-negative integer range. */
function byteCount(value) {
    return Math.min(Number.MAX_SAFE_INTEGER, Math.max(0, Math.round(value)));
}
/**
 * Report usage with room to grow: half again what the caches hold, and never less room than the
 * standalone allowance, so joining a host never shrinks a page's caches below it.
 */
export function usageMessage(session, usage) {
    const used = byteCount(usage.usedBytes);
    const consumers = usage.consumers
        .filter((consumer) => CONSUMER_ID.test(consumer.id))
        .slice(0, MAX_CONSUMERS)
        .map((consumer) => ({ id: consumer.id, bytes: byteCount(consumer.bytes) }));
    return {
        protocol: MEMORY_PROTOCOL,
        version: MEMORY_PROTOCOL_VERSION,
        session,
        type: 'usage',
        usedBytes: used,
        demandBytes: byteCount(used + Math.max(DEFAULT_MEMORY_ALLOWANCE_BYTES, Math.round(used / 2))),
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
/** Accept only a schema-valid allowance addressed to `session`. */
export function readAllowance(value, session) {
    if (!isMemoryAllowanceMessage(value) || value.session !== session)
        return null;
    return {
        generation: value.generation,
        level: value.level,
        allowanceBytes: value.allowanceBytes,
        ceilingBytes: value.ceilingBytes,
        totalBytes: value.totalBytes,
        preference: value.preference,
    };
}
