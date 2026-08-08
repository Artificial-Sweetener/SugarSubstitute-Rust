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
/** Adapt the active LiteGraph scene palette into Cube surface colors. */

import type { CubeNodeColorTheme } from './CubeNodeColorTheme.js';

interface LiteGraphNodeColorSource {
  NODE_DEFAULT_COLOR?: unknown;
  NODE_DEFAULT_BGCOLOR?: unknown;
  NODE_SELECTED_TITLE_COLOR?: unknown;
  NODE_TITLE_COLOR?: unknown;
}

const LITEGRAPH_NATIVE_DEFAULT_THEME: CubeNodeColorTheme = {
  header: '#333',
  body: '#353535',
};

/** Resolve one Cube shell from explicit node colors and the live LiteGraph palette. */
export function resolveComfyLiteGraphCubeSurfaceTheme(
  node: object,
  source: LiteGraphNodeColorSource | null | undefined = globalThis.LiteGraph,
): CubeNodeColorTheme {
  return {
    header:
      readColor(Reflect.get(node, 'color')) ??
      readColor(source?.NODE_DEFAULT_COLOR) ??
      LITEGRAPH_NATIVE_DEFAULT_THEME.header,
    body:
      readColor(Reflect.get(node, 'bgcolor')) ??
      readColor(source?.NODE_DEFAULT_BGCOLOR) ??
      LITEGRAPH_NATIVE_DEFAULT_THEME.body,
  };
}

/** Resolve the live LiteGraph title-text token used as the model-pill fill. */
export function resolveComfyLiteGraphTitleTextColor(
  source: LiteGraphNodeColorSource | null | undefined = globalThis.LiteGraph,
): string {
  return (
    readColor(source?.NODE_SELECTED_TITLE_COLOR) ?? readColor(source?.NODE_TITLE_COLOR) ?? '#f0f2f5'
  );
}

/** Narrow one dynamic host color to a non-empty Canvas color string. */
function readColor(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}
