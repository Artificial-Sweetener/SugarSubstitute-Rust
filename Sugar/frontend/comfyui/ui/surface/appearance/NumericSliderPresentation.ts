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
/** Select bounded numeric presentation without changing native value or seed policies. */
import type { VisualStyle } from './VisualStyle.js';

export interface NumericSliderSpec {
  name: string;
  label: string;
  precision: number | null;
  minimum: number;
  maximum: number;
  step: number;
  presentation?: 'spinner-slider';
}

/** Use GIMP spin scales for bounded properties and retain the other providers' established float policy. */
export function usesNumericSlider(field: NumericSliderSpec, style: VisualStyle): boolean {
  const bounded =
    [field.minimum, field.maximum, field.step].every(Number.isFinite) &&
    field.maximum > field.minimum &&
    field.step > 0 &&
    (field.maximum - field.minimum) / field.step <= 2_147_483_647;
  if (!bounded) return false;
  // GIMP's size-entry dimensions use ordinary spinbuttons, while tool properties use spin scales.
  if (style === 'gimp') return !['width', 'height'].includes(field.name);
  return usesSliderSpinner(field);
}

/** Preserve conventional slider-and-spinner fields when selecting GIMP's numeric control variant. */
export function usesGimpSpinScale(field: NumericSliderSpec): boolean {
  return usesNumericSlider(field, 'gimp') && !usesSliderSpinner(field);
}

/** Share the established slider-and-spinner semantics across visual-style providers. */
function usesSliderSpinner(field: NumericSliderSpec): boolean {
  const label = field.label.trim().toLowerCase().replace(/[_-]/g, ' ').replace(/\s+/g, ' ');
  return (
    field.presentation === 'spinner-slider' ||
    (field.precision !== 0 &&
      (['denoise', 'variation_strength'].includes(field.name) || label === 'scale factor'))
  );
}
