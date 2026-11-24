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
  menuTokenDeclarations,
} from '../../../frontend/comfyui/ui/menus/MenuTokens.js';
import { contextMenuCss } from '../../../frontend/comfyui/ui/menus/ContextMenuStyles.js';
import {
  comfyMenuCss,
  comfyMenuThemeCss,
} from '../../../frontend/comfyui/ui/surface/frame/ComfyMenuStyles.js';

const STYLES = Object.keys(MENU_LOOKS) as (keyof typeof MENU_LOOKS)[];

/** The custom properties one rule body declares. */
function declarations(css: string): Map<string, string> {
  const body = css.slice(css.indexOf('{') + 1, css.lastIndexOf('}'));
  const declared = new Map<string, string>();
  for (const match of body.matchAll(/(--[\w-]+)\s*:\s*([^;]*);/g))
    declared.set(match[1] ?? '', (match[2] ?? '').trim());
  return declared;
}

/** Whether a value resolves to something in a scope that declares only `declared`. */
function resolves(value: string, declared: Map<string, string>, seen = new Set<string>()): boolean {
  const reference = /var\(\s*(--[\w-]+)\s*(?:,(.*))?\)\s*$/.exec(value.trim());
  if (!value.includes('var(')) return value.trim() !== '';
  if (!reference) return true;
  const [, name = '', fallback] = reference;
  const own = declared.get(name);
  if (own !== undefined && !seen.has(name) && resolves(own, declared, new Set([...seen, name])))
    return true;
  return fallback !== undefined && resolves(fallback, declared, seen);
}

test('every style supplies every menu token', () => {
  for (const style of STYLES) {
    for (const token of MENU_TOKENS) expect(MENU_LOOKS[style][token]).toBeTruthy();
    for (const token of MENU_TOKENS)
      expect(menuTokenDeclarations(style)).toContain(`--sc-menu-${token}:`);
  }
});

test('ComfyUI’s menus carry their own token scope, and every token resolves in it', () => {
  // ComfyUI's menus sit outside every SugarCubes surface: a token that only resolves inside one
  // left their hover, text and separators empty.
  for (const style of STYLES)
    for (const dark of [true, false]) {
      const css = comfyMenuThemeCss(style, dark);
      expect(css.startsWith(':root[data-sc-substitute-frame] :is(.p-contextmenu,')).toBe(true);
      expect(css).toContain('.litecontextmenu)');
      const declared = declarations(css);
      for (const token of MENU_TOKENS) {
        const value = declared.get(`--sc-menu-${token}`) ?? '';
        expect([style, dark, token, resolves(value, declared)]).toEqual([style, dark, token, true]);
      }
      for (const menu of ['contextmenu', 'tieredmenu', 'menu'])
        expect(declared.get(`--p-${menu}-item-focus-background`)).toBe('var(--sc-menu-row-hover)');
    }
  expect(comfyMenuThemeCss('comfy', true)).toBe('');
});

test('ComfyUI’s graph and PrimeVue menus read the tokens, and only in the frame', () => {
  const css = comfyMenuCss.replace(/\s+/g, ' ');
  expect(css).toContain(':root[data-sc-substitute-frame] .litecontextmenu {');
  expect(css).toContain('background:var(--sc-menu-row-hover) !important');
  const rules = css
    .split('}')
    .map((part) => part.trim())
    .filter((part) => part.includes('{'));
  for (const rule of rules) expect(rule.startsWith(':root[data-sc-substitute-frame]')).toBe(true);
});

test('a disabled Fluent menu row stays on the menu surface', () => {
  expect(contextMenuCss.replace(/\s+/g, ' ')).toContain(
    'html[data-sc-visual-style="fluent"] .sc-context-menu fluent-menu-item:is([disabled],[aria-disabled="true"]) { background-color:transparent; }',
  );
});
