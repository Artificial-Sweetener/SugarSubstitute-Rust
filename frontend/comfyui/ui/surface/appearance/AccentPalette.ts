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
/** Own qfluentwidgets-compatible HSV variants and the control roles derived from them. */

/** Retain the configured seed independently of mode-dependent presentation. */
export const DEFAULT_ACCENT_COLOR = '#479ef5';

/** Seven independent shades derived from one configured RGB seed and resolved mode. */
export interface AccentVariants {
  readonly primary: string;
  readonly dark1: string;
  readonly dark2: string;
  readonly dark3: string;
  readonly light1: string;
  readonly light2: string;
  readonly light3: string;
}

/** Colors for an enabled control's pointer states. */
export interface AccentStates {
  readonly rest: string;
  readonly hover: string;
  readonly pressed: string;
}

/** Resolve semantic roles once so renderers never invent their own accent shades. */
export interface AccentPalette {
  readonly variants: AccentVariants;
  readonly button: AccentStates;
  readonly checkbox: AccentStates;
  readonly toggle: AccentStates;
  readonly link: AccentStates;
  readonly indicator: string;
  readonly selection: string;
  readonly foreground: string;
  readonly pressedForeground: string;
  readonly buttonBorder: string;
  readonly buttonBottomBorder: string;
  readonly buttonHoverBorder: string;
}

/** Accept opaque hexadecimal colors without admitting arbitrary stylesheet text. */
export function normalizeAccentColor(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const color = value.trim().toLowerCase();
  if (/^#[\da-f]{6}$/.test(color)) return color;
  if (/^#[\da-f]{3}$/.test(color))
    return `#${Array.from(color.slice(1), (character) => character + character).join('')}`;
  return null;
}

/** Match ThemeColor.color(): HSV maths use doubles between Qt's float/16-bit boundaries. */
export function accentVariants(color: string, dark: boolean): AccentVariants {
  const normalized = normalizeAccentColor(color);
  if (!normalized) throw new TypeError('Accent colors must be opaque hexadecimal colors.');
  const channel = (offset: number): number =>
    Number.parseInt(normalized.slice(offset, offset + 2), 16);
  const [h, saturation, value] = rgbToHsv(channel(1), channel(3), channel(5));
  const s = dark ? saturation * 0.84 : saturation;
  const v = dark ? 1 : value;
  const shade = (saturationScale: number, valueScale: number): string =>
    hsvToHex(h, s * saturationScale, v * valueScale);
  return {
    primary: shade(1, 1),
    dark1: shade(1, dark ? 0.9 : 0.75),
    dark2: shade(dark ? 0.977 : 1.05, dark ? 0.82 : 0.5),
    dark3: shade(dark ? 0.95 : 1.1, dark ? 0.7 : 0.4),
    light1: shade(dark ? 0.92 : 1, dark ? 1 : 1.05),
    light2: shade(dark ? 0.78 : 0.75, dark ? 1 : 1.05),
    light3: shade(0.65, dark ? 1 : 1.05),
  };
}

/** Assign each qfluentwidgets control its own mode-dependent interaction shades. */
export function createAccentPalette(color: string, dark: boolean): AccentPalette {
  const variants = accentVariants(color, dark);
  const { primary, dark1, dark2, light1, light2, light3 } = variants;
  return {
    variants,
    button: { rest: primary, hover: dark ? dark1 : light1, pressed: dark ? dark2 : light3 },
    checkbox: { rest: primary, hover: dark ? dark1 : light1, pressed: dark ? dark2 : light2 },
    toggle: { rest: primary, hover: light1, pressed: light2 },
    link: { rest: primary, hover: primary, pressed: dark ? dark1 : primary },
    indicator: primary,
    selection: dark ? primary : light1,
    foreground: dark ? '#000000' : '#ffffff',
    pressedForeground: dark ? 'rgba(0,0,0,0.63)' : 'rgba(255,255,255,0.63)',
    buttonBorder: light1,
    buttonBottomBorder: dark ? light2 : dark1,
    buttonHoverBorder: dark ? light1 : light2,
  };
}

/** Quantize QColor's HSV representation before exposing its single-precision components. */
function rgbToHsv(red: number, green: number, blue: number): readonly [number, number, number] {
  const f = Math.fround;
  const r = f(red / 255),
    g = f(green / 255),
    b = f(blue / 255);
  const maximum = Math.max(r, g, b),
    minimum = Math.min(r, g, b);
  const delta = f(maximum - minimum);
  if (delta === 0) return [-1, 0, maximum];
  let sector =
    r === maximum
      ? f(f(g - b) / delta)
      : g === maximum
        ? f(2 + f(f(b - r) / delta))
        : f(4 + f(f(r - g) / delta));
  sector = f(sector * 60);
  if (sector < 0) sector = f(sector + 360);
  return [
    Math.round(f(sector * 100)),
    f(Math.round(f(f(delta / maximum) * 65535)) / 65535),
    maximum,
  ];
}

/** Preserve Qt's float operations, 16-bit channel rounding and final 8-bit conversion. */
function hsvToHex(hue: number, saturation: number, value: number): string {
  const f = Math.fround;
  const s = f(Math.round(f(f(Math.min(saturation, 1)) * 65535)) / 65535);
  const v = f(Math.round(f(f(Math.min(value, 1)) * 65535)) / 65535);
  let channels: readonly number[];
  if (hue < 0 || s === 0) channels = [v, v, v];
  else {
    const h = hue === 36000 ? 0 : f(hue / 6000);
    const sector = Math.floor(h),
      fraction = f(h - sector);
    const p = f(v * f(1 - s));
    const q = f(v * f(1 - f(s * fraction)));
    const t = f(v * f(1 - f(s * f(1 - fraction))));
    switch (sector) {
      case 0:
        channels = [v, t, p];
        break;
      case 1:
        channels = [q, v, p];
        break;
      case 2:
        channels = [p, v, t];
        break;
      case 3:
        channels = [p, q, v];
        break;
      case 4:
        channels = [t, p, v];
        break;
      default:
        channels = [v, p, q];
    }
  }
  return (
    '#' +
    channels
      .map((channel) =>
        Math.floor((Math.round(f(channel * 65535)) + 128) / 257)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  );
}
