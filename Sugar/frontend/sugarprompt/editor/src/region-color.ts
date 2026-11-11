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
/** Prepared regional colors consume the shared ordinal palette and add Qt-compatible native hover lightening. */
import {
  REGION_PALETTE_BASE,
  formatRegionHex,
  regionPaletteColor,
  type RegionRgb,
} from '../../core/src/index.js';

/** Both immutable declarations belong to one source-prepared ordinal palette, not native hover work. */
export interface PreparedRegionColor {
  readonly normal: string;
  readonly hover: string;
}

/** Read the shared palette's channels for one ordinal of the related separators. */
function regionChannels(ordinal: number, total: number, base: RegionRgb): readonly number[] {
  const { r, g, b } = regionPaletteColor(base, ordinal, total);
  return [r, g, b];
}

/** Qt6.11.2 QColor.lighter135 preserves16-bit HSV rounding and desaturates value overflow before RGB conversion. */
function hoverChannels(channels: readonly number[]): readonly number[] {
  const [red, green, blue] = channels.map((channel) => channel * 257) as [number, number, number],
    maximum = Math.max(red, green, blue),
    minimum = Math.min(red, green, blue),
    chroma = maximum - minimum,
    f = Math.fround;
  let hue = 65535,
    saturation = 0;
  if (chroma) {
    saturation = Math.round(f(f(chroma / maximum) * 65535));
    let sector =
      maximum === red
        ? f((green - blue) / chroma)
        : maximum === green
          ? f(2 + f((blue - red) / chroma))
          : f(4 + f((red - green) / chroma));
    if (sector < 0) sector = f(sector + 6);
    hue = Math.round(f(sector * 6000));
  }
  const amplified = Math.floor((maximum * 135) / 100),
    value = Math.min(65535, amplified);
  saturation = Math.max(0, saturation - Math.max(0, amplified - 65535));
  if (!saturation) return [value, value, value].map((channel) => Math.round(channel / 257));
  const position = hue === 36000 ? 0 : f(hue / 6000),
    sector = Math.floor(position),
    fraction = f(position - sector),
    s = f(saturation / 65535),
    v = f(value / 65535),
    low = f(v * f(1 - s)),
    middle = sector % 2 ? f(v * f(1 - f(s * fraction))) : f(v * f(1 - f(s * f(1 - fraction))));
  const output =
    sector === 0
      ? [v, middle, low]
      : sector === 1
        ? [middle, v, low]
        : sector === 2
          ? [low, v, middle]
          : sector === 3
            ? [low, middle, v]
            : sector === 4
              ? [middle, low, v]
              : [v, low, middle];
  return output.map((channel) => Math.round(Math.round(f(channel * 65535)) / 257));
}

/** Prepare exact normal and hover CSS once with source projection; the host accent is the palette base. */
export function prepareRegionColor(
  ordinal: number,
  total: number,
  base: RegionRgb = REGION_PALETTE_BASE,
): PreparedRegionColor {
  const channels = regionChannels(ordinal, total, base);
  return Object.freeze({
    normal: ordinal === 0 ? formatRegionHex(base) : `rgb(${channels.join(', ')})`,
    hover: `rgb(${hoverChannels(channels).join(', ')})`,
  });
}
