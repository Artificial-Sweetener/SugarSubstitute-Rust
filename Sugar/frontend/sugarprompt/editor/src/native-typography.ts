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
/** Measurement-phase typography adapts the regional label font to its finite native draft. */

/** Explicit properties preserve font features that do not serialize into a CSS font shorthand. */
export interface InputTypography {
  readonly fontFamily: string;
  readonly fontSize: string;
  readonly fontStyle: string;
  readonly fontWeight: string;
  readonly fontStretch: string;
  readonly fontVariant: string;
  readonly fontVariationSettings: string;
  readonly fontKerning: string;
  readonly fontFeatureSettings: string;
  readonly letterSpacing: string;
}

/** Read only inside the caller's native measurement phase. */
export function measuredTypography(element: Element): InputTypography {
  const style = getComputedStyle(element);
  return {
    fontFamily: style.fontFamily,
    fontSize: style.fontSize,
    fontStyle: style.fontStyle,
    fontWeight: style.fontWeight,
    fontStretch: style.fontStretch,
    fontVariant: style.fontVariant,
    fontVariationSettings: style.fontVariationSettings,
    fontKerning: style.fontKerning,
    fontFeatureSettings: style.fontFeatureSettings,
    letterSpacing: style.letterSpacing,
  };
}
