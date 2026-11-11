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
 * Own surface-value persistence policy for SugarCube UI state.
 */
import { isRecord } from '../types/common.js';
function readControlString(control, key) {
    const value = isRecord(control) ? control[key] : undefined;
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Return whether one surface control represents live seed runtime state.
 */
export function isVolatileSeedControl(control) {
    return readControlString(control, 'input_name') === 'seed';
}
/**
 * Return controls whose values are tracked by flavors and dirty comparison.
 */
export function trackedSurfaceControls(surface) {
    const controls = Array.isArray(surface?.controls) ? surface.controls : [];
    return controls.filter((control) => !isVolatileSeedControl(control));
}
/**
 * Remove volatile seed values from a persisted surface-value map.
 */
export function filterTrackedSurfaceValues(surface, values) {
    const lookup = isRecord(values) ? values : {};
    const volatileIds = new Set((Array.isArray(surface?.controls) ? surface.controls : [])
        .filter((control) => isVolatileSeedControl(control))
        .map((control) => readControlString(control, 'control_id'))
        .filter(Boolean));
    const filtered = {};
    for (const [controlId, value] of Object.entries(lookup)) {
        if (volatileIds.has(controlId)) {
            continue;
        }
        filtered[controlId] = value;
    }
    return filtered;
}
