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
/** Prove a desktop host's first launch selects Fluent and Vue nodes once and keeps later choices. */
import { jest } from '@jest/globals';
import {
  DESKTOP_DEFAULTS_SETTING,
  DesktopPresentationDefaults,
} from '../../../frontend/comfyui/ui/surface/frame/DesktopPresentationDefaults.js';

/** A settings store persisting across application lifetimes, as Comfy's does on disk. */
function settings(stored: Record<string, unknown> = {}) {
  const defaults: Record<string, unknown> = {};
  return {
    stored,
    addSetting: jest.fn((setting: { id: string; defaultValue: unknown; type: string }) => {
      defaults[setting.id] = setting.defaultValue;
    }),
    getSettingValue: (id: string): unknown => stored[id] ?? defaults[id],
    setSettingValue: jest.fn(async (id: string, value: unknown) => {
      stored[id] = value;
    }),
  };
}

/** Let the queued setting writes and their recording settle. */
async function settled(): Promise<void> {
  for (let turn = 0; turn < 10; turn++) await Promise.resolve();
}

test('the first desktop launch selects Fluent and Vue nodes and records that it did', async () => {
  const store = settings({ 'Comfy.VueNodes.Enabled': false });
  const report = jest.fn();
  const defaults = new DesktopPresentationDefaults({ ui: { settings: store } }, report);
  expect(store.addSetting).not.toHaveBeenCalled();
  defaults.apply();
  defaults.apply();
  await settled();
  expect(store.addSetting).toHaveBeenCalledTimes(1);
  expect(store.addSetting).toHaveBeenCalledWith(
    expect.objectContaining({ id: DESKTOP_DEFAULTS_SETTING, type: 'hidden' }),
  );
  expect(store.stored).toEqual({
    'SugarCubes.Appearance.VisualStyle': 'fluent',
    'Comfy.VueNodes.Enabled': true,
    [DESKTOP_DEFAULTS_SETTING]: 1,
  });
  expect(report).not.toHaveBeenCalled();
});

test('a later launch keeps whatever the person chose after the defaults were applied', async () => {
  const store = settings({
    'SugarCubes.Appearance.VisualStyle': 'spectrum',
    'Comfy.VueNodes.Enabled': false,
    [DESKTOP_DEFAULTS_SETTING]: 1,
  });
  const report = jest.fn();
  new DesktopPresentationDefaults({ ui: { settings: store } }, report).apply();
  await settled();
  expect(store.setSettingValue).not.toHaveBeenCalled();
  expect(store.stored['SugarCubes.Appearance.VisualStyle']).toBe('spectrum');
  expect(report).not.toHaveBeenCalled();
});

test('a failed write is reported and leaves the defaults to be applied at the next launch', async () => {
  const store = settings();
  const failure = new Error('settings store unavailable');
  store.setSettingValue.mockRejectedValueOnce(failure);
  const report = jest.fn();
  new DesktopPresentationDefaults({ ui: { settings: store } }, report).apply();
  await settled();
  expect(report).toHaveBeenCalledWith(failure);
  expect(store.stored[DESKTOP_DEFAULTS_SETTING]).toBeUndefined();
  const missing = jest.fn();
  new DesktopPresentationDefaults({ ui: {} }, missing).apply();
  expect(missing).toHaveBeenCalledTimes(1);
});
