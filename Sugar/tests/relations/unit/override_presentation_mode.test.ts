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
/** Prove Plugin Mode precedence and persistence without a second set of override controls. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { OverridePresentationHost } from '../../../frontend/comfyui/ui/relations/infrastructure/OverridePresentationHost.js';
import { PLUGIN_MODE_SETTING } from '../../../frontend/comfyui/ui/surface/frame/PluginModePreference.js';
import type {
  GlobalOverrideToolbar,
  OverrideToolbarPresentation,
} from '../../../frontend/comfyui/ui/relations/presentation/GlobalOverrideToolbar.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

afterEach(() => document.documentElement.removeAttribute('data-sc-plugin-mode'));

test('Plugin Mode stays first in presentation precedence and restores the current frame choice', () => {
  const presentations: OverrideToolbarPresentation[] = [];
  const toolbar: Pick<GlobalOverrideToolbar, 'setPresentation' | 'reconcile'> = {
    setPresentation: (presentation) => presentations.push(presentation),
    reconcile: jest.fn(),
  };
  let change: (value: unknown) => void = () => {
    throw new Error('Preference not registered');
  };
  const register = jest.fn((setting: unknown) => {
    if (!isRecord(setting) || typeof setting.onChange !== 'function')
      throw new Error('Invalid Plugin Mode setting');
    expect(setting.id).toBe(PLUGIN_MODE_SETTING);
    expect(setting.defaultValue).toBe(false);
    const callback = setting.onChange;
    change = (value) => callback(value);
  });
  const report = jest.fn();
  const host = new OverridePresentationHost(toolbar);
  host.setup(
    { ui: { settings: { addSetting: register, getSettingValue: () => true } } },
    document,
    report,
  );
  expect(presentations.at(-1)).toBe('plugin');
  expect(document.documentElement.hasAttribute('data-sc-plugin-mode')).toBe(true);
  host.setInline(true);
  expect(presentations.at(-1)).toBe('plugin');
  change(false);
  expect(presentations.at(-1)).toBe('substitute');
  host.setInline(false);
  expect(presentations.at(-1)).toBe('comfy');
  change(true);
  change('invalid');
  expect(presentations.at(-1)).toBe('plugin');
  expect(report).toHaveBeenCalledTimes(1);
  host.dispose();
  expect(document.documentElement.hasAttribute('data-sc-plugin-mode')).toBe(false);
});
