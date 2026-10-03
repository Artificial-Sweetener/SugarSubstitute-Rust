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
/** Match SugarSubstitute's bounded decimal spinner/slider presentation. */

export interface NumericSliderSpec {
  name: string;
  label: string;
  precision: number | null;
  minimum: number;
  maximum: number;
  step: number;
}

/** Select Substitute's float controls without turning seeds or arbitrary numbers into sliders. */
export function usesSubstituteSlider(field: NumericSliderSpec): boolean {
  const label = field.label.trim().toLowerCase().replace(/[_-]/g, ' ').replace(/\s+/g, ' ');
  return (
    field.precision !== 0 &&
    (['denoise', 'variation_strength'].includes(field.name) || label === 'scale factor') &&
    [field.minimum, field.maximum, field.step].every(Number.isFinite) &&
    field.maximum > field.minimum &&
    field.step > 0 &&
    (field.maximum - field.minimum) / field.step <= 2_147_483_647
  );
}
