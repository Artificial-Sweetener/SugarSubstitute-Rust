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
/** Derive distinguishable mask overlay colours from the theme accent in 8-bit HSV terms. */

/** Overlay opacity given to masks whose node has no stored visual opacity. */
export const DEFAULT_MASK_OPACITY = 0.5;

interface Hsv {
  /** Degrees 0..359, or -1 for achromatic colours. */
  readonly h: number;
  readonly s: number;
  readonly v: number;
}

/**
 * Return the overlay colour of mask `index` among `count` masks sharing one surface.
 *
 * Mask 0 keeps the accent unchanged. Later masks take the accent's saturation and value raised to
 * at least 200 and 220 and rotate its hue: two masks are complementary, three take the complement
 * ±30°, four step by 90° and five or more divide the circle evenly.
 */
export function maskColor(accent: string, index: number, count: number): string {
  const base = parseHexColor(accent);
  if (!base) throw new Error('A mask colour requires a #rrggbb accent.');
  if (index <= 0) return formatHexColor(base);
  const { h, s, v } = rgbToHsv(base);
  return formatHexColor(
    hsvToRgb({ h: maskColorHue(h, index, count), s: Math.max(s, 200), v: Math.max(v, 220) }),
  );
}

/** Rotate the base hue for one mask; an achromatic base behaves like hue 359. */
export function maskColorHue(baseHue: number, index: number, count: number): number {
  const base = ((baseHue % 360) + 360) % 360;
  if (index <= 0) return base;
  if (count === 2) return (base + 180) % 360;
  if (count === 3) {
    const complement = (base + 180) % 360;
    return index === 1 ? (complement + 330) % 360 : (complement + 30) % 360;
  }
  if (count === 4) return (base + index * 90) % 360;
  if (count >= 5) return Math.trunc((base + index * (360 / count)) % 360);
  return base;
}

/** Accept only opaque six-digit hexadecimal colours. */
export function parseHexColor(
  value: string,
): { readonly r: number; readonly g: number; readonly b: number } | null {
  const match = /^#([\da-f]{2})([\da-f]{2})([\da-f]{2})$/i.exec(value.trim());
  if (!match?.[1] || !match[2] || !match[3]) return null;
  return {
    r: Number.parseInt(match[1], 16),
    g: Number.parseInt(match[2], 16),
    b: Number.parseInt(match[3], 16),
  };
}

/** Format channels as the lowercase `#rrggbb` wire form. */
function formatHexColor({ r, g, b }: { r: number; g: number; b: number }): string {
  return '#' + [r, g, b].map((channel) => channel.toString(16).padStart(2, '0')).join('');
}

/** Convert to integer HSV with 0..255 saturation and value. */
function rgbToHsv({ r, g, b }: { r: number; g: number; b: number }): Hsv {
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
function hsvToRgb({ h, s, v }: Hsv): { r: number; g: number; b: number } {
  if (h < 0 || s === 0) return { r: v, g: v, b: v };
  const saturation = s / 255,
    chroma = v * saturation,
    sector = (h % 360) / 60,
    x = chroma * (1 - Math.abs((sector % 2) - 1)),
    m = v - chroma;
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
  return { r: Math.round(r + m), g: Math.round(g + m), b: Math.round(b + m) };
}
