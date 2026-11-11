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
/** Express output identities in the host surface contract's identity space. */
/**
 * Name one output image to the native host. The surface contract admits only `result:` identities,
 * but an image restored from a desktop session keeps its durable artifact UUID as its page identity
 * so retained-media lookup stays exact. The host treats the identity as an opaque key, so the prefix
 * carries the UUID into the contract's space without ever colliding with a live job's JSON identity.
 */
export function nativeCanvasItemId(id) {
    return isResultIdentity(id) ? id : `result:${id}`;
}
/** Whether an identity already lives in the host contract's `result:` space. */
function isResultIdentity(id) {
    return id.startsWith('result:');
}
