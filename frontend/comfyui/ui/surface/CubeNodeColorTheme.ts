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
/** Resolve native Cube card colors and their darker enclosing backdrop. */

export interface CubeNodeColorTheme {
  header: string;
  body: string;
}

interface RgbaColor {
  red: number;
  green: number;
  blue: number;
  alpha: number;
}

const FALLBACK_BACKDROP = '#0d1117';
const BACKDROP_BRIGHTNESS = 0.7;

/** Return the complete explicit theme written by Comfy's node-color action. */
export function resolveCubeNodeColorTheme(node: object): CubeNodeColorTheme | null {
  const header = readColor(Reflect.get(node, 'color'));
  const body = readColor(Reflect.get(node, 'bgcolor'));
  return header && body ? { header, body } : null;
}

/** Render the enclosing Cube surface at 70% of the selected native body brightness. */
export function deriveCubeBackdropColor(body: string): string {
  const color = parseColor(body);
  if (!color) return FALLBACK_BACKDROP;
  return serializeColor({
    red: Math.round(color.red * BACKDROP_BRIGHTNESS),
    green: Math.round(color.green * BACKDROP_BRIGHTNESS),
    blue: Math.round(color.blue * BACKDROP_BRIGHTNESS),
    alpha: color.alpha,
  });
}

/** Accept only non-empty host color strings. */
function readColor(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}

/** Parse the CSS color forms emitted by Comfy's native color palettes. */
function parseColor(value: string): RgbaColor | null {
  const normalized = value.trim();
  const hex = parseHexColor(normalized);
  if (hex) return hex;
  const rgb = normalized.match(/^rgba?\(([^)]+)\)$/i);
  if (!rgb?.[1]) return null;
  const channels = rgb[1]
    .split(/[\s,/]+/)
    .filter(Boolean)
    .map((channel) => Number(channel));
  if (
    channels.length < 3 ||
    channels.length > 4 ||
    channels.some((channel) => !Number.isFinite(channel))
  ) {
    return null;
  }
  const [red, green, blue, alpha = 1] = channels;
  if (red === undefined || green === undefined || blue === undefined) return null;
  return {
    red: clampByte(red),
    green: clampByte(green),
    blue: clampByte(blue),
    alpha: Math.min(1, Math.max(0, alpha)),
  };
}

/** Parse short and long hexadecimal colors with optional alpha. */
function parseHexColor(value: string): RgbaColor | null {
  const match = value.match(/^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i);
  if (!match?.[1]) return null;
  const digits = match[1];
  const expanded =
    digits.length <= 4 ? digits.replace(/./g, (digit) => `${digit}${digit}`) : digits;
  return {
    red: Number.parseInt(expanded.slice(0, 2), 16),
    green: Number.parseInt(expanded.slice(2, 4), 16),
    blue: Number.parseInt(expanded.slice(4, 6), 16),
    alpha: expanded.length === 8 ? Number.parseInt(expanded.slice(6, 8), 16) / 255 : 1,
  };
}

/** Serialize a Canvas-compatible color without adding alpha to opaque values. */
function serializeColor(color: RgbaColor): string {
  if (color.alpha >= 1) {
    return `rgb(${String(color.red)} ${String(color.green)} ${String(color.blue)})`;
  }
  return `rgb(${String(color.red)} ${String(color.green)} ${String(color.blue)} / ${String(color.alpha)})`;
}

/** Keep a parsed channel inside the finite CSS byte range. */
function clampByte(value: number): number {
  return Math.round(Math.min(255, Math.max(0, value)));
}
