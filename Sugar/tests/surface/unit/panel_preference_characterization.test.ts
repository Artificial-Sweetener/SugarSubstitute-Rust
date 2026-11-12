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
/** Preserve independent saved presentation preferences before adding embedded sessions. */
import { expect, jest, test } from '@jest/globals';
import {
  registerPluginModePreference,
  PLUGIN_MODE_SETTING,
} from '../../../frontend/comfyui/ui/surface/frame/PluginModePreference.js';
import { readPluginMode } from '../../../frontend/comfyui/ui/core/PluginMode.js';
import {
  registerVisualStylePreference,
  VISUAL_STYLE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/VisualStylePreference.js';
import {
  registerPhotoProPalettePreference,
  PHOTOPRO_PALETTE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/PhotoProPalettePreference.js';

test('restores each saved preference without writing settings or coupling mode and style', () => {
  const values = new Map<string, unknown>([
    [PLUGIN_MODE_SETTING, true],
    [VISUAL_STYLE_SETTING, 'fluent'],
    [PHOTOPRO_PALETTE_SETTING, 'black'],
  ]);
  const changes = new Map<string, (value: unknown) => void>();
  const settings = {
    addSetting: (setting: { id: string; onChange(value: unknown): void }): void => {
      changes.set(setting.id, setting.onChange);
    },
    getSettingValue: (key: string): unknown => values.get(key),
    setSettingValue: jest.fn(),
  };
  const app = { ui: { settings } };
  const mode = jest.fn();
  const style = jest.fn();
  const report = jest.fn();
  const stopMode = registerPluginModePreference(app, document, mode, report);
  registerVisualStylePreference(app, style, report);
  const stopPalette = registerPhotoProPalettePreference(app, document, report);
  expect(readPluginMode(document)).toBe(true);
  expect(style).toHaveBeenLastCalledWith('fluent');
  expect(document.documentElement.dataset.scPhotoproPalette).toBe('black');
  changes.get(VISUAL_STYLE_SETTING)?.('photopro');
  expect(style).toHaveBeenLastCalledWith('photopro');
  expect(mode).toHaveBeenCalledTimes(1);
  changes.get(PLUGIN_MODE_SETTING)?.(false);
  expect(readPluginMode(document)).toBe(false);
  expect(style).toHaveBeenCalledTimes(2);
  expect(settings.setSettingValue).not.toHaveBeenCalled();
  expect(report).not.toHaveBeenCalled();
  stopMode();
  stopPalette();
});
