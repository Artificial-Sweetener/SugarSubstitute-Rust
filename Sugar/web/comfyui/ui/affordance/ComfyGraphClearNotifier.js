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
/** Notify Comfy stores after a Cube-root clear through the host event boundary. */
import { isRecord } from '../types/common.js';
/** Dispatch the stable graph-cleared event or leave an actionable compatibility warning. */
export function notifyComfyGraphCleared(value, logger) {
    const dispatch = isRecord(value) ? value.dispatchCustomEvent : null;
    if (typeof dispatch !== 'function') {
        logger.warn('SugarCubes: Comfy graph-cleared event integration is unavailable.');
        return;
    }
    dispatch.call(value, 'graphCleared');
}
