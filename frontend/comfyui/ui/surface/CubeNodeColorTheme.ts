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

import { parseCssColor, serializeCssColor } from './CssColor.js';

export interface CubeNodeColorTheme {
  header: string;
  body: string;
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
  const color = parseCssColor(body);
  if (!color) return FALLBACK_BACKDROP;
  return serializeCssColor({
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
