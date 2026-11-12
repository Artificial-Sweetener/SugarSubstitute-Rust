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
/** Generated protocol validation; edit the authoritative host.schema.json. */
import type * as Contract from '../contracts/HostProtocol.generated.js';
/** Narrow external records without allowing arrays. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
/** Validate a bounded HostCapability payload before dispatch. */
export function isHostCapability(value: unknown): value is Contract.HostCapability {
  return (
    isRecord(value) &&
    Object.keys(value).length === 1 &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['name'])
  );
}
/** Validate a bounded HostProtocol payload before dispatch. */
export function isHostProtocol(value: unknown): value is Contract.HostProtocol {
  return (
    isRecord(value) &&
    Object.keys(value).length === 2 &&
    Object.hasOwn(value, 'name') &&
    typeof value['name'] === 'string' &&
    value['name'].length <= 64 &&
    new RegExp('^[a-z0-9.-]{1,64}$').test(value['name']) &&
    Object.hasOwn(value, 'version') &&
    typeof value['version'] === 'number' &&
    Number.isSafeInteger(value['version']) &&
    value['version'] >= 1 &&
    value['version'] <= 65535
  );
}
/** Validate a bounded HostDescriptionMessage payload before dispatch. */
export function isHostDescriptionMessage(value: unknown): value is Contract.HostDescriptionMessage {
  return (
    isRecord(value) &&
    Object.keys(value).length === 6 &&
    Object.hasOwn(value, 'protocol') &&
    value['protocol'] === 'sugarcubes.host' &&
    Object.hasOwn(value, 'version') &&
    value['version'] === 1 &&
    Object.hasOwn(value, 'session') &&
    typeof value['session'] === 'string' &&
    value['session'].length <= 80 &&
    new RegExp('^[a-zA-Z0-9-]{16,80}$').test(value['session']) &&
    Object.hasOwn(value, 'type') &&
    value['type'] === 'description' &&
    Object.hasOwn(value, 'protocols') &&
    Array.isArray(value['protocols']) &&
    value['protocols'].length <= 32 &&
    value['protocols'].every((item: unknown) => isHostProtocol(item)) &&
    Object.hasOwn(value, 'capabilities') &&
    Array.isArray(value['capabilities']) &&
    value['capabilities'].length <= 64 &&
    value['capabilities'].every((item: unknown) => isHostCapability(item))
  );
}
/** Validate a bounded HostMessage payload before dispatch. */
export function isHostMessage(value: unknown): value is Contract.HostMessage {
  return isHostDescriptionMessage(value);
}
