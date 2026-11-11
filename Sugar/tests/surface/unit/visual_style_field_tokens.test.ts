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
/** Prove shared fields resolve to each provider's actual geometry and focus policy in both palettes. */
import { expect, test } from '@jest/globals';
import { visualStyleTokens } from '../../../frontend/comfyui/ui/surface/appearance/ThemeService.js';

/** Resolve the single-token aliases used by provider geometry without emulating a browser CSS engine. */
function tokenValue(declarations: string, name: string): string {
  const tokens = new Map<string, string>();
  for (const match of declarations.matchAll(/(--[\w-]+):([^;]*);/g)) {
    const key = match[1];
    const value = match[2];
    if (key && value !== undefined) tokens.set(key, value.trim());
  }
  const visited = new Set<string>();
  let key = name;
  while (!visited.has(key)) {
    visited.add(key);
    const value = tokens.get(key);
    if (value === undefined) throw new Error(`Provider is missing ${key}`);
    const alias = /^var\((--[\w-]+)\)$/.exec(value)?.[1];
    if (!alias) return value;
    key = alias;
  }
  throw new Error(`Provider token cycle at ${key}`);
}

test.each([true, false])(
  'Spectrum fields use the official two-pixel border and external focus policy, dark=%s',
  (dark) => {
    const tokens = visualStyleTokens('spectrum', dark);
    expect(tokenValue(tokens, '--sc-field-border-width')).toBe('2px');
    expect(tokenValue(tokens, '--sc-field-radius')).toBe('8px');
    expect(tokenValue(tokens, '--sc-field-focus-outline-offset')).toBe('2px');
    expect(tokenValue(tokens, '--sc-field-stroke')).toBe(
      tokenValue(tokens, '--system-textfield-border-color'),
    );
    expect(tokenValue(tokens, '--sc-field-focus-stroke')).toBe(
      tokenValue(tokens, '--system-textfield-border-color-focus'),
    );
    expect(tokenValue(tokens, '--sc-field-focus-shadow')).toBe('none');
    expect(tokenValue(tokens, '--sc-field-underline-display')).toBe('none');
    expect(tokenValue(tokens, '--sc-button-border-width')).toBe('2px');
    expect(tokenValue(tokens, '--sc-button-fill')).toBe(
      tokenValue(tokens, '--system-button-background-color-default'),
    );
    expect(tokenValue(tokens, '--sc-line-height')).toBe('1.3');
  },
);

test.each([true, false])(
  'Fluent retains its established field geometry and underline, dark=%s',
  (dark) => {
    const tokens = visualStyleTokens('fluent', dark);
    expect(tokenValue(tokens, '--sc-field-border-width')).toBe('1px');
    expect(tokenValue(tokens, '--sc-field-radius')).toBe('4px');
    expect(tokenValue(tokens, '--sc-field-focus-shadow')).toBe(
      'inset 0 -1px var(--sc-brand-border)',
    );
    expect(tokenValue(tokens, '--sc-field-underline-display')).toBe('block');
    expect(tokenValue(tokens, '--sc-button-radius')).toBe('4px');
    expect(tokenValue(tokens, '--sc-button-border-width')).toBe('1px');
    expect(tokenValue(tokens, '--sc-line-height')).toBe('20px');
  },
);
