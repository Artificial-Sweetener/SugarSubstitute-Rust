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
/** Prove stack persistence, external setting updates and failed-write behavior. */
import { expect, jest, test } from '@jest/globals';
import {
  CubeStackPreferences,
  CUBE_STACK_VISIBLE_SETTING,
  CUBE_STACK_COMPACT_SETTING,
} from '../../../frontend/comfyui/ui/surface/stack/CubeStackPreferences.js';

test('restores validated preferences and registers each setting once', () => {
  const fixture = setup();
  fixture.values.set(CUBE_STACK_VISIBLE_SETTING, false);
  fixture.values.set(CUBE_STACK_COMPACT_SETTING, true);
  fixture.preferences.setup();
  fixture.preferences.setup();
  expect(fixture.preferences.snapshot).toEqual({ visible: false, compact: true });
  expect(fixture.registered.size).toBe(2);
  expect(Object.isFrozen(fixture.preferences.snapshot)).toBe(true);
  expect(fixture.write).not.toHaveBeenCalled();
});

test('publishes accepted external values once and ignores malformed host values', () => {
  const fixture = setup();
  fixture.preferences.setup();
  const changed = jest.fn();
  fixture.preferences.subscribe(changed);
  fixture.registered.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(false);
  fixture.registered.get(CUBE_STACK_VISIBLE_SETTING)?.onChange(false);
  fixture.registered.get(CUBE_STACK_COMPACT_SETTING)?.onChange('true');
  expect(fixture.preferences.snapshot).toEqual({ visible: false, compact: false });
  expect(changed).toHaveBeenCalledTimes(1);
});

test('persists control intent and leaves state unchanged when persistence fails', async () => {
  const fixture = setup();
  fixture.preferences.setup();
  await fixture.preferences.setCompact(true);
  expect(fixture.write).toHaveBeenCalledWith(CUBE_STACK_COMPACT_SETTING, true);
  fixture.write.mockRejectedValue(new Error('Settings unavailable'));
  await expect(fixture.preferences.setVisible(false)).rejects.toThrow('Settings unavailable');
  expect(fixture.preferences.snapshot).toEqual({ visible: true, compact: true });
});

test('retires late persistence and host callbacks after teardown', async () => {
  const fixture = setup();
  fixture.preferences.setup();
  let finish: (() => void) | undefined;
  fixture.write.mockImplementation(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const changed = jest.fn();
  fixture.preferences.subscribe(changed);
  const writing = fixture.preferences.setVisible(false);
  fixture.preferences.dispose();
  finish?.();
  await writing;
  fixture.registered.get(CUBE_STACK_COMPACT_SETTING)?.onChange(true);
  expect(fixture.preferences.snapshot).toEqual({ visible: true, compact: false });
  expect(changed).not.toHaveBeenCalled();
  await expect(fixture.preferences.setCompact(true)).rejects.toThrow('unavailable');
});

test('rejects incomplete persistence capability without registering partial settings', () => {
  const preferences = new CubeStackPreferences({ ui: { settings: { addSetting: jest.fn() } } });
  expect(() => preferences.setup()).toThrow('settings persistence');
});

/** Supply an actual setting-callback boundary with isolated durable values. */
function setup() {
  const values = new Map<string, unknown>();
  const registered = new Map<string, { onChange(value: unknown): void }>();
  const write = jest.fn(async (id: string, value: boolean) => {
    values.set(id, value);
  });
  const preferences = new CubeStackPreferences({
    ui: {
      settings: {
        addSetting(setting: { id: string; onChange(value: unknown): void }) {
          registered.set(setting.id, setting);
        },
        getSettingValue(id: string, fallback: boolean) {
          return values.get(id) ?? fallback;
        },
        setSettingValue: write,
      },
    },
  });
  return { preferences, values, registered, write };
}
