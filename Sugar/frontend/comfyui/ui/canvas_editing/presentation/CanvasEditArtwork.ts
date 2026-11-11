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
/**
 * Address the vendored editing artwork from this module's own deployed location.
 *
 * The build keeps this module unbundled at its canonical path, so `import.meta.url` resolves the
 * relative artwork paths against `comfyui/ui/canvas_editing/presentation/` wherever the Vue
 * chrome that renders the icons is bundled.
 */
/** Microsoft Fluent UI System Icons, copied verbatim at the pinned upstream revision. */
const FLUENT = [
  'pan-zoom',
  'move',
  'brush',
  'eraser',
  'select-rectangle',
  'select-lasso',
  'transform',
  'shared-edges',
  'undo',
  'redo',
  'rotate-left',
  'rotate-right',
  'flip-horizontal',
  'flip-vertical',
] as const;
/** SugarSubstitute's Fluent-derived shape artwork and QFluent's dialog artwork. */
const SUBSTITUTE = [
  'select-ellipse',
  'mask-rectangle',
  'mask-ellipse',
  'mask-lasso',
  'deselect',
  'apply',
  'close',
  'modify',
] as const;

export type CanvasEditIconName = (typeof FLUENT)[number] | (typeof SUBSTITUTE)[number];

/** Every icon the editing chrome can render. */
export const CANVAS_EDIT_ICON_NAMES: readonly CanvasEditIconName[] = Object.freeze([
  ...FLUENT,
  ...SUBSTITUTE,
]);

const URLS: ReadonlyMap<CanvasEditIconName, string> = new Map<CanvasEditIconName, string>([
  ...FLUENT.map(
    (name) =>
      [
        name,
        new URL(`../../../../icons/canvas-editing/${name}.svg`, import.meta.url).href,
      ] as const,
  ),
  ...SUBSTITUTE.map(
    (name) =>
      [name, new URL(`../../../../canvas-editing/${name}.svg`, import.meta.url).href] as const,
  ),
]);

/** The served URL of one icon's artwork. */
export function canvasEditIconUrl(name: CanvasEditIconName): string {
  return URLS.get(name) ?? '';
}
