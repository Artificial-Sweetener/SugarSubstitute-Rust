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
/** Select bounded compound presentation for native numeric fields. */
/** Honor explicit compound fields while retaining the established automatic float selection. */
export function usesSubstituteSlider(field) {
    const label = field.label.trim().toLowerCase().replace(/[_-]/g, ' ').replace(/\s+/g, ' ');
    return ((field.presentation === 'spinner-slider' ||
        (field.precision !== 0 &&
            (['denoise', 'variation_strength'].includes(field.name) || label === 'scale factor'))) &&
        [field.minimum, field.maximum, field.step].every(Number.isFinite) &&
        field.maximum > field.minimum &&
        field.step > 0 &&
        (field.maximum - field.minimum) / field.step <= 2_147_483_647);
}
