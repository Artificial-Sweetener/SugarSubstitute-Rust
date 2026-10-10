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
/** Validate accent inputs and derive a complete brand ramp with readable foregrounds. */

export type AccentBrandStep =
  | 10
  | 20
  | 30
  | 40
  | 50
  | 60
  | 70
  | 80
  | 90
  | 100
  | 110
  | 120
  | 130
  | 140
  | 150
  | 160;
export type AccentBrandPalette = Record<AccentBrandStep, string>;
/** Retain the current bright blue as the initial user accent. */
export const DEFAULT_ACCENT_COLOR = '#479ef5';

/** Accept opaque hexadecimal colors without admitting arbitrary stylesheet text. */
export function normalizeAccentColor(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const color = value.trim().toLowerCase();
  if (/^#[\da-f]{6}$/.test(color)) return color;
  if (/^#[\da-f]{3}$/.test(color))
    return `#${Array.from(color.slice(1), (character) => character + character).join('')}`;
  return null;
}

/** Blend validated RGB colors for the ordered slots consumed by Microsoft's theme factory. */
export function createAccentBrandPalette(color: string): AccentBrandPalette {
  return {
    10: mixAccentColor(color, '#000000', 0.94),
    20: mixAccentColor(color, '#000000', 0.86),
    30: mixAccentColor(color, '#000000', 0.75),
    40: mixAccentColor(color, '#000000', 0.62),
    50: mixAccentColor(color, '#000000', 0.48),
    60: mixAccentColor(color, '#000000', 0.32),
    70: mixAccentColor(color, '#000000', 0.16),
    80: color,
    90: mixAccentColor(color, '#ffffff', 0.1),
    100: mixAccentColor(color, '#ffffff', 0.2),
    110: mixAccentColor(color, '#ffffff', 0.32),
    120: mixAccentColor(color, '#ffffff', 0.44),
    130: mixAccentColor(color, '#ffffff', 0.56),
    140: mixAccentColor(color, '#ffffff', 0.68),
    150: mixAccentColor(color, '#ffffff', 0.8),
    160: mixAccentColor(color, '#ffffff', 0.92),
  };
}

/** Select the higher-contrast black or white foreground using relative luminance. */
export function accentForeground(color: string): string {
  const luminance = relativeLuminance(color);
  return (luminance + 0.05) / 0.05 >= 1.05 / (luminance + 0.05) ? '#000000' : '#ffffff';
}

/** Keep small indicators and focus marks legible on the host's neutral surface. */
export function accentIndicatorColor(color: string, dark: boolean): string {
  const surface = relativeLuminance(dark ? '#2b2b2b' : '#ffffff');
  for (let step = 0; step <= 20; step += 1) {
    const candidate = mixAccentColor(color, dark ? '#ffffff' : '#000000', step / 20);
    const luminance = relativeLuminance(candidate);
    if ((Math.max(surface, luminance) + 0.05) / (Math.min(surface, luminance) + 0.05) >= 4.5)
      return candidate;
  }
  return dark ? '#ffffff' : '#000000';
}

/** Interpolate validated channel values without depending on browser color parsing. */
export function mixAccentColor(color: string, destination: string, weight: number): string {
  const [red, green, blue] = readChannels(color);
  const [targetRed, targetGreen, targetBlue] = readChannels(destination);
  const pairs: ReadonlyArray<readonly [number, number]> = [
    [red, targetRed],
    [green, targetGreen],
    [blue, targetBlue],
  ];
  return `#${pairs
    .map(([channel, target]) =>
      Math.round(channel * (1 - weight) + target * weight)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`;
}

/** Convert an sRGB channel to linear light for relative luminance comparisons. */
function linearChannel(channel: number): number {
  const normalized = channel / 255;
  return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
}

/** Measure linear light before comparing text or indicator contrast. */
function relativeLuminance(color: string): number {
  const [red, green, blue] = readChannels(color);
  return linearChannel(red) * 0.2126 + linearChannel(green) * 0.7152 + linearChannel(blue) * 0.0722;
}

/** Reject invalid callers before values reach palette math or generated stylesheet text. */
function readChannels(color: string): readonly [number, number, number] {
  const normalized = normalizeAccentColor(color);
  if (!normalized) throw new TypeError('Accent colors must be opaque hexadecimal colors.');
  return [
    Number.parseInt(normalized.slice(1, 3), 16),
    Number.parseInt(normalized.slice(3, 5), 16),
    Number.parseInt(normalized.slice(5, 7), 16),
  ];
}
