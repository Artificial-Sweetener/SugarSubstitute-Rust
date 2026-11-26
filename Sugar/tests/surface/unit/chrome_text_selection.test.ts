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
/** In the desktop frame chrome text is unselectable while edited and read content stays selectable. */
import { expect, test } from '@jest/globals';
import {
  chromeTextSelectionCss,
  SELECTABLE_CONTENT,
} from '../../../frontend/comfyui/ui/surface/frame/ChromeTextSelectionStyles.js';

test('chrome text cannot be selected in the frame, but fields, editors, logs and dialogs can', () => {
  const css = chromeTextSelectionCss.replace(/\s+/g, ' ');
  expect(css).toContain(
    ':root[data-sc-substitute-frame] body { user-select:none; -webkit-user-select:none; }',
  );
  for (const content of [
    'input',
    'textarea',
    '.cm-editor',
    '.sc-console__output',
    '.sugarcubes-modal__panel',
    '.p-dialog-content',
  ])
    expect(SELECTABLE_CONTENT).toContain(content);
  expect(css).toContain(`:is(${SELECTABLE_CONTENT.join(', ')}) { user-select:text;`);
  // The rules apply only under the desktop frame; the browser keeps ComfyUI's own behaviour.
  for (const rule of css.split('}').filter((part) => part.includes('{')))
    expect(rule.trim().startsWith(':root[data-sc-substitute-frame]')).toBe(true);
});
