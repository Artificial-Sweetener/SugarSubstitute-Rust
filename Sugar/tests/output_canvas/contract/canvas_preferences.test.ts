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
/** Prove the output canvas registers only its global visibility default with Comfy settings. */

import { jest } from '@jest/globals';
import {
  ComfyCanvasPreferences,
  OUTPUT_CANVAS_VISIBLE_SETTING,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasPreferences.js';

interface Setting {
  readonly id: string;
  readonly type: string;
  readonly defaultValue: unknown;
}

test('registers the visibility default once and leaves editor widths to workflow views', () => {
  const registered = new Map<string, Setting>();
  const write = jest.fn();
  const preferences = new ComfyCanvasPreferences({
    ui: {
      settings: {
        addSetting: (setting: Setting) => registered.set(setting.id, setting),
        getSettingValue: jest.fn(),
        setSettingValueAsync: write,
      },
    },
  });
  preferences.setup();
  preferences.setup();
  expect([...registered.keys()]).toEqual([OUTPUT_CANVAS_VISIBLE_SETTING]);
  expect(registered.get(OUTPUT_CANVAS_VISIBLE_SETTING)).toMatchObject({
    type: 'boolean',
    defaultValue: true,
  });
  expect(write).not.toHaveBeenCalled();
});

test('fails clearly when Comfy settings registration is unavailable', () => {
  expect(() => new ComfyCanvasPreferences({}).setup()).toThrow('unavailable');
  expect(() => new ComfyCanvasPreferences({ ui: { settings: {} } }).setup()).toThrow('unavailable');
});
