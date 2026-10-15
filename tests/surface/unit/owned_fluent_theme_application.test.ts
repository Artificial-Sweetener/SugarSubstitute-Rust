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
/** Prove owned themes avoid repeat writes while preserving real token and connection changes. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { webDarkTheme, webLightTheme } from '@fluentui/tokens';
import { applyOwnedFluentTheme } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('applies changed themes and releases them without rewriting an accepted palette', () => {
  const element = document.createElement('section');
  document.body.append(element);
  applyOwnedFluentTheme(element, true);
  expect(element.style.getPropertyValue('--colorNeutralBackground1')).toBe(
    webDarkTheme.colorNeutralBackground1,
  );
  const writes = jest.spyOn(CSSStyleDeclaration.prototype, 'setProperty');
  for (let index = 0; index < 10; index++) applyOwnedFluentTheme(element, true);
  expect(writes).not.toHaveBeenCalled();
  applyOwnedFluentTheme(element, false);
  expect(writes).toHaveBeenCalled();
  expect(element.style.getPropertyValue('--colorNeutralBackground1')).toBe(
    webLightTheme.colorNeutralBackground1,
  );
  const removals = jest.spyOn(CSSStyleDeclaration.prototype, 'removeProperty');
  applyOwnedFluentTheme(element, null);
  expect(removals).toHaveBeenCalled();
  removals.mockClear();
  applyOwnedFluentTheme(element, null);
  expect(removals).not.toHaveBeenCalled();
});

test('does not remember a theme that the native API could not apply before connection', () => {
  const element = document.createElement('section');
  applyOwnedFluentTheme(element, true);
  expect(element.style.getPropertyValue('--colorNeutralBackground1')).toBe('');
  document.body.append(element);
  applyOwnedFluentTheme(element, true);
  expect(element.style.getPropertyValue('--colorNeutralBackground1')).toBe(
    webDarkTheme.colorNeutralBackground1,
  );
  element.remove();
  applyOwnedFluentTheme(element, null);
  document.body.append(element);
  applyOwnedFluentTheme(element, false);
  expect(element.style.getPropertyValue('--colorNeutralBackground1')).toBe(
    webLightTheme.colorNeutralBackground1,
  );
});
