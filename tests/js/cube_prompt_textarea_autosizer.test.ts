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
/** Verify authoritative Cube prompt textarea browser sizing. */

import { describe, expect, jest, test } from '@jest/globals';

import { CubePromptTextareaAutosizer } from '../../frontend/comfyui/ui/surface/CubePromptTextareaAutosizer.js';

describe('CubePromptTextareaAutosizer', () => {
  test('owns content height and restores exact inline styles on release', () => {
    const textarea = document.createElement('textarea');
    textarea.style.setProperty('height', '30px', 'important');
    textarea.style.overflowY = 'auto';
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 168 });
    const onHeightChange = jest.fn();
    const autosizer = new CubePromptTextareaAutosizer(document);

    autosizer.bind(textarea, onHeightChange);

    expect(textarea.style.height).toBe('168px');
    expect(textarea.style.overflowY).toBe('hidden');
    expect(textarea.style.getPropertyPriority('height')).toBe('important');
    expect(Object.hasOwn(textarea, 'value')).toBe(true);
    expect(onHeightChange).toHaveBeenCalledWith(168);

    autosizer.unbind(textarea);

    expect(textarea.style.height).toBe('30px');
    expect(textarea.style.getPropertyPriority('height')).toBe('important');
    expect(textarea.style.overflowY).toBe('auto');
    expect(Object.hasOwn(textarea, 'value')).toBe(false);
    autosizer.dispose();
  });

  test('remeasures property-only workflow hydration through the native value setter', () => {
    const textarea = document.createElement('textarea');
    let contentHeight = 1;
    Object.defineProperty(textarea, 'scrollHeight', {
      configurable: true,
      get: () => contentHeight,
    });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);
    expect(textarea.style.height).toBe('1px');

    contentHeight = 148;
    textarea.value = 'hydrated multiline workflow prompt';

    expect(textarea.style.height).toBe('148px');
    autosizer.dispose();
  });

  test('remeasures subsequent editor input immediately', () => {
    const textarea = document.createElement('textarea');
    let contentHeight = 80;
    Object.defineProperty(textarea, 'scrollHeight', {
      configurable: true,
      get: () => contentHeight,
    });
    const autosizer = new CubePromptTextareaAutosizer(document);
    autosizer.bind(textarea);

    contentHeight = 220;
    textarea.dispatchEvent(new Event('input'));

    expect(textarea.style.height).toBe('220px');
    autosizer.dispose();
  });
});
