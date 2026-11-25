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
 * The page's copy of the shared native and web UI element record
 * (`integrations/contracts/ui/elements.json`): icons served from `icons/shared`, the caption
 * button's size and fills, and the graph/masonry toggle's two layouts. A contract test pins these
 * values to the record the native host reads too.
 */

/** A shared icon, named by what activating it does: a toggle shows where it goes. */
export type SharedIconName =
  | 'console-open'
  | 'console-close'
  | 'canvas-open'
  | 'canvas-close'
  | 'window-close';

/** The deployed URL of one shared icon. */
export function sharedIconUrl(name: SharedIconName): string {
  return new URL(`../../../icons/shared/${name}.svg`, import.meta.url).href;
}

/**
 * A window caption button: flat, no rounding, and no held fill; alphas are out of 255. The caption
 * row starts `top` pixels below the window's edge, where the window's own buttons are drawn.
 */
export const CAPTION_BUTTON = {
  width: 46,
  height: 32,
  top: 1,
  glyph: 16,
  hover: 26,
  pressed: 51,
} as const;

/** One box on the 20-unit icon grid: left, top, right, bottom. */
export type IconBox = readonly [number, number, number, number];

/** The graph/masonry toggle's artwork: four outlined nodes and the links between them. */
export const WORKFLOW_VIEW_ICON = {
  outline: 1,
  cornerRadius: 1.5,
  link: 0.8,
  durationMs: 240,
  linkRetract: 2.2,
  graph: [
    [3, 3, 7.8, 7],
    [6.6, 12.4, 12, 17],
    [12.2, 5, 17, 9],
    [21.5, 11.4, 26.3, 15.4],
  ] as readonly IconBox[],
  masonry: [
    [3, 3, 9.5, 8],
    [3, 9, 9.5, 17],
    [10.5, 3, 17, 12],
    [10.5, 13, 17, 17],
  ] as readonly IconBox[],
  swing: [3.2, 3.0, 2.8] as readonly number[],
} as const;

/** CSS for one caption-row button following the shared caption button record. */
export function captionButtonCss(selector: string): string {
  const { width, height, glyph, hover, pressed } = CAPTION_BUTTON;
  const percent = (alpha: number): string => `${((alpha / 255) * 100).toFixed(2)}%`;
  return `
    ${selector} { display:inline-flex; align-items:center; justify-content:center; box-sizing:border-box;
      width:${String(width)}px; height:${String(height)}px; padding:0; margin:0; border:0; border-radius:0;
      background:transparent; color:var(--fg-color,#fff); cursor:default; -webkit-app-region:no-drag; }
    ${selector}:hover { background:color-mix(in srgb,currentColor ${percent(hover)},transparent); }
    ${selector}:active { background:color-mix(in srgb,currentColor ${percent(pressed)},transparent); }
    ${selector}:focus-visible { outline:2px solid var(--sc-accent-color,currentColor); outline-offset:-2px; }
    ${selector} > span { width:${String(glyph)}px; height:${String(glyph)}px; background:currentColor;
      pointer-events:none; mask:var(--sc-caption-icon) center / contain no-repeat; }`;
}
