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
/** Generate opaque browser session identities without requiring a secure-context-only UUID API. */
/** Draw a 128-bit identity using the Web Crypto capability available to ordinary Comfy HTTP hosts. */
export function workflowSessionKey() {
    const words = crypto.getRandomValues(new Uint32Array(4));
    return [...words].map((word) => word.toString(16).padStart(8, '0')).join('');
}
