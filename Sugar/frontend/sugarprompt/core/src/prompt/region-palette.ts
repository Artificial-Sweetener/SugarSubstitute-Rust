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
/** One ordinal palette colours every member of a related region or mask set in 8-bit HSV terms. */

/** Opaque 8-bit sRGB channels. */
export interface RegionRgb {
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

/** The base colour of SugarPrompt's regional rails, shared by every view of the same regions. */
export const REGION_PALETTE_BASE: RegionRgb = Object.freeze({ r: 79, g: 109, b: 245 });

/**
 * Rotate the base hue for member `ordinal` of `total` related members.
 *
 * Two members are complementary, three take the complement ±30°, four step by 90° and five or
 * more divide the circle evenly. Member 0, and any member of a single-member set, keeps the base.
 */
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

/**
 * Return member `ordinal` of `total` related members derived from `base`.
 *
 * Member 0 keeps the base unchanged. Later members take the base's saturation and value raised to
 * at least 200 and 220 and the rotated hue; an achromatic base behaves like hue 359.
 */
export function regionPaletteColor(base: RegionRgb, ordinal: number, total: number): RegionRgb {
  if (ordinal <= 0) return Object.freeze({ ...base });
  const { h, s, v } = rgbToHsv(base);
  return hsvToRgb(regionHue(h, ordinal, total), Math.max(s, 200), Math.max(v, 220));
}

/** Accept only opaque six-digit hexadecimal colours. */
export function parseRegionHex(value: string): RegionRgb | null {
  const match = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(value.trim());
  if (!match?.[1] || !match[2] || !match[3]) return null;
  return Object.freeze({
    r: Number.parseInt(match[1], 16),
    g: Number.parseInt(match[2], 16),
    b: Number.parseInt(match[3], 16),
  });
}

/** Format channels as lowercase `#rrggbb`. */
export function formatRegionHex({ r, g, b }: RegionRgb): string {
  return '#' + [r, g, b].map((channel) => channel.toString(16).padStart(2, '0')).join('');
}

/** Convert to integer HSV with 0..255 saturation and value; achromatic colours report hue -1. */
function rgbToHsv({ r, g, b }: RegionRgb): { h: number; s: number; v: number } {
  const max = Math.max(r, g, b),
    min = Math.min(r, g, b),
    delta = max - min;
  if (delta === 0) return { h: -1, s: 0, v: max };
  let hue: number;
  if (max === r) hue = ((g - b) / delta) % 6;
  else if (max === g) hue = (b - r) / delta + 2;
  else hue = (r - g) / delta + 4;
  return {
    h: Math.round((hue * 60 + 360) % 360) % 360,
    s: Math.round((delta * 255) / max),
    v: max,
  };
}

/** Convert integer HSV back to rounded 8-bit channels. */
function hsvToRgb(h: number, s: number, v: number): RegionRgb {
  const saturation = s / 255,
    value = v / 255,
    sector = (h % 360) / 60,
    chroma = saturation * value,
    x = chroma * (1 - Math.abs((sector % 2) - 1)),
    minimum = value - chroma;
  const [r, g, b] =
    sector < 1
      ? [chroma, x, 0]
      : sector < 2
        ? [x, chroma, 0]
        : sector < 3
          ? [0, chroma, x]
          : sector < 4
            ? [0, x, chroma]
            : sector < 5
              ? [x, 0, chroma]
              : [chroma, 0, x];
  return Object.freeze({
    r: Math.round((r + minimum) * 255),
    g: Math.round((g + minimum) * 255),
    b: Math.round((b + minimum) * 255),
  });
}
