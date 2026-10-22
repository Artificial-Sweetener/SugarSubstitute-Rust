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
/** Prepared regional colors retain the pinned ordinal hues and Qt-compatible native hover lightening. */

/** Related regional rails use complementary, split-complementary, square or evenly spaced hues. */
export function regionHue(base: number, ordinal: number, total: number): number {
  const hue = ((base % 360) + 360) % 360;
  const offset =
    ordinal <= 0
      ? 0
      : total === 2
        ? 180
        : total === 3
          ? ordinal === 1
            ? 150
            : 210
          : total === 4
            ? ordinal * 90
            : total >= 5
              ? (ordinal * 360) / total
              : 0;
  return Math.floor((hue + offset) % 360);
}

/** Both immutable declarations belong to one source-prepared ordinal palette, not native hover work. */
export interface PreparedRegionColor {
  readonly normal: string;
  readonly hover: string;
}

/** Preserve the port's existing normal palette before applying the independently qualified Qt operation. */
function regionChannels(ordinal: number, total: number): readonly number[] {
  if (ordinal === 0) return [79, 109, 245];
  const hue = regionHue(229, ordinal, total) / 60,
    saturation = 200 / 255,
    value = 245 / 255,
    chroma = saturation * value,
    x = chroma * (1 - Math.abs((hue % 2) - 1)),
    minimum = value - chroma;
  const channels =
    hue < 1
      ? [chroma, x, 0]
      : hue < 2
        ? [x, chroma, 0]
        : hue < 3
          ? [0, chroma, x]
          : hue < 4
            ? [0, x, chroma]
            : hue < 5
              ? [x, 0, chroma]
              : [chroma, 0, x];
  return channels.map((channel) => Math.round((channel + minimum) * 255));
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

/** Prepare exact normal and hover CSS once with source projection; the current native accent remains the palette input. */
export function prepareRegionColor(ordinal: number, total: number): PreparedRegionColor {
  const channels = regionChannels(ordinal, total);
  return Object.freeze({
    normal: ordinal === 0 ? '#4f6df5' : `rgb(${channels.join(', ')})`,
    hover: `rgb(${hoverChannels(channels).join(', ')})`,
  });
}
