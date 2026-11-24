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
/** One menu look per style, shared by SugarCubes' menus and, in the frame, ComfyUI's. */
import { expect, test } from '@jest/globals';
import {
  MENU_LOOKS,
  MENU_TOKENS,
  menuTokenCss,
} from '../../../frontend/comfyui/ui/menus/MenuTokens.js';
import { contextMenuCss } from '../../../frontend/comfyui/ui/menus/ContextMenuStyles.js';
import { comfyMenuCss } from '../../../frontend/comfyui/ui/surface/frame/ComfyMenuStyles.js';

test('every style supplies every menu token', () => {
  for (const [style, look] of Object.entries(MENU_LOOKS)) {
    for (const token of MENU_TOKENS) expect(look[token]).toBeTruthy();
    expect(menuTokenCss()).toContain(`html[data-sc-visual-style="${style}"] {`);
  }
});

test('ComfyUI’s graph and PrimeVue menus read the tokens, and only in the frame', () => {
  const css = comfyMenuCss.replace(/\s+/g, ' ');
  for (const menu of ['contextmenu', 'tieredmenu', 'menu'])
    expect(css).toContain(`--p-${menu}-item-focus-background:var(--sc-menu-row-hover);`);
  expect(css).toContain(':root[data-sc-substitute-frame] .litecontextmenu {');
  expect(css).toContain('background:var(--sc-menu-row-hover) !important');
  const rules = css
    .split('}')
    .map((part) => part.trim())
    .filter((part) => part.includes('{') && !part.startsWith('html[data-sc-visual-style='));
  for (const rule of rules) expect(rule.startsWith(':root[data-sc-substitute-frame]')).toBe(true);
});

test('a disabled Fluent menu row stays on the menu surface', () => {
  expect(contextMenuCss.replace(/\s+/g, ' ')).toContain(
    'html[data-sc-visual-style="fluent"] .sc-context-menu fluent-menu-item:is([disabled],[aria-disabled="true"]) { background-color:transparent; }',
  );
});
