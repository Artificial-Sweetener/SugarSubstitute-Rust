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
/** Classify template-like icon tones and invert only neutral visible pixels for readable appearance. */
const NEUTRAL_SPREAD_LIMIT = 18;
const NEUTRAL_CONFIDENCE = 0.88;
const VISIBLE_ALPHA = 16;

/** Identify an asset's neutral polarity while retaining colored and mixed-tone artwork. */
export type CubeIconTone = 'bright' | 'dark' | 'original';

/** Sample already decoded RGBA pixels without depending on DOM, canvas or host appearance. */
export function classifyCubeIconTone(pixels: Uint8ClampedArray): CubeIconTone {
  if (pixels.length % 4 !== 0)
    throw new TypeError('Icon pixels must contain complete RGBA samples.');
  let visible = 0;
  let neutral = 0;
  let luminance = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    const red = pixels[index] ?? 0;
    const green = pixels[index + 1] ?? 0;
    const blue = pixels[index + 2] ?? 0;
    if ((pixels[index + 3] ?? 0) < VISIBLE_ALPHA) continue;
    visible += 1;
    if (Math.max(red, green, blue) - Math.min(red, green, blue) <= NEUTRAL_SPREAD_LIMIT)
      neutral += 1;
    luminance += red * 0.2126 + green * 0.7152 + blue * 0.0722;
  }
  if (visible === 0 || neutral / visible < NEUTRAL_CONFIDENCE) return 'original';
  const mean = luminance / visible;
  return mean >= 170 ? 'bright' : mean <= 85 ? 'dark' : 'original';
}

/** Return an independent contrast variant with identical alpha and unchanged colored details. */
export function invertNeutralCubeIconPixels(pixels: Uint8ClampedArray): Uint8ClampedArray {
  if (pixels.length % 4 !== 0)
    throw new TypeError('Icon pixels must contain complete RGBA samples.');
  const adjusted = new Uint8ClampedArray(pixels);
  for (let index = 0; index < pixels.length; index += 4) {
    const red = pixels[index] ?? 0;
    const green = pixels[index + 1] ?? 0;
    const blue = pixels[index + 2] ?? 0;
    if (
      (pixels[index + 3] ?? 0) === 0 ||
      Math.max(red, green, blue) - Math.min(red, green, blue) > NEUTRAL_SPREAD_LIMIT
    )
      continue;
    adjusted[index] = 255 - red;
    adjusted[index + 1] = 255 - green;
    adjusted[index + 2] = 255 - blue;
  }
  return adjusted;
}
