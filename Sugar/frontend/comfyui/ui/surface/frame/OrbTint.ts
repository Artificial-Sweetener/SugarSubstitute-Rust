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
/** Preserve Substitute's luminance shading and translucent glass highlights in web composition. */
export type OrbRgb = readonly [number, number, number];

/** Transform one source pixel with the original glass-highlight and accent-shading formulas. */
export function tintOrbPixel(
  source: readonly [number, number, number, number],
  accent: OrbRgb,
): readonly [number, number, number, number] {
  const [red, green, blue, alphaChannel] = source;
  if (!alphaChannel) return [0, 0, 0, 0];
  const luminance = Math.round(red * 0.299 + green * 0.587 + blue * 0.114) / 255;
  const smooth = (value: number): number => {
    const x = Math.max(0, Math.min(1, value));
    return x * x * (3 - 2 * x);
  };
  const highlight = Math.min(
    1,
    smooth((luminance - 0.8) / 0.2) ** 1.02 * (1 - (alphaChannel / 255) ** 5) * 1.18,
  );
  const shade = 0.58 + 0.42 * smooth(luminance);
  const channel = (value: number): number =>
    Math.round(value * shade + (255 - value * shade) * highlight);
  return [channel(accent[0]), channel(accent[1]), channel(accent[2]), alphaChannel];
}
