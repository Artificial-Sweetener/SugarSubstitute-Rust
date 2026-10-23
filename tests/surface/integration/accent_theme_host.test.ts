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
/** Prove one palette owner persists settings and follows host CSS without touching graph inventory. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  AccentThemeHost,
  ACCENT_COLOR_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/AccentThemeHost.js';
import { DEFAULT_ACCENT_COLOR } from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

interface ColorSetting {
  id: string;
  category: string[];
  type: () => HTMLElement;
  onChange: (value: unknown) => void;
}

import {
  EmbeddedPresentation,
  type EmbeddedPresentationSource,
} from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';

const hosts: AccentThemeHost[] = [];
afterEach(() => {
  hosts.splice(0).forEach((host) => host.dispose());
  document.documentElement.removeAttribute('style');
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Bind a real observer and stylesheet to a narrow Comfy settings boundary. */
function createHost(saved: unknown = '#b58cff', embedded?: EmbeddedPresentationSource) {
  let setting: ColorSetting | undefined;
  const setDirty = jest.fn();
  const report = jest.fn();
  const setSettingValue = jest.fn<(id: string, value: string) => void>((_id, value) =>
    setting!.onChange(value),
  );
  const host = new AccentThemeHost(
    document,
    {
      canvas: { setDirty },
      ui: {
        settings: {
          getSettingValue: () => saved,
          addSetting: (value: ColorSetting) => {
            setting = value;
          },
          setSettingValue,
        },
      },
    },
    report,
    embedded,
  );
  hosts.push(host);
  host.setup();
  return { host, setting: setting!, setDirty, report, setSettingValue };
}

/** Allow native mutation delivery and the owner's coalesced palette task to settle. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

test('loads the saved color without rewriting it, then updates through the registered menu', () => {
  const { setting, setDirty, setSettingValue, host } = createHost();
  expect(setting.id).toBe(ACCENT_COLOR_SETTING);
  expect(setting.category).toEqual(['SugarCubes', 'Appearance', 'AccentColor']);
  expect(setSettingValue).not.toHaveBeenCalled();
  const style = document.getElementById('sugarcubes-accent-theme')!;
  expect(style.textContent).toContain('--sc-accent-color:#b58cff');
  const row = setting.type();
  const input = row.querySelector<HTMLInputElement>('input[type="text"]')!;
  input.value = '#071530';
  input.dispatchEvent(new Event('change'));
  expect(setSettingValue).toHaveBeenCalledWith(ACCENT_COLOR_SETTING, '#071530');
  expect(style.textContent).toContain('--sc-accent-foreground:#ffffff');
  expect(style.textContent).toContain('--sc-accent-color:#071530');
  expect(style.textContent).toContain('--sc-brand-fill:var(--sc-accent-color) !important');
  expect(setDirty).toHaveBeenCalledTimes(2);
  host.refresh();
  host.setup();
  expect(setDirty).toHaveBeenCalledTimes(2);
});

test('external root CSS wins until removed, preserves user choices, and never rebuilds graph inventory', async () => {
  const inventory = jest.fn();
  document.addEventListener('sugarcubes-native-node-card-change', inventory);
  const { setting, setDirty, setSettingValue } = createHost();
  document.documentElement.style.setProperty('--sugarcubes-accent', '#ffe066');
  await settle();
  const style = document.getElementById('sugarcubes-accent-theme')!;
  expect(style.textContent).toContain('--sc-accent-color:#ffe066');
  expect(style.textContent).toContain('--sc-accent-foreground:#000000');
  setting.onChange('#123456');
  expect(style.textContent).toContain('--sc-accent-color:#ffe066');
  document.documentElement.style.removeProperty('--sugarcubes-accent');
  await settle();
  expect(style.textContent).toContain('--sc-accent-color:#123456');
  expect(setDirty).toHaveBeenCalledTimes(3);
  expect(inventory).not.toHaveBeenCalled();
  expect(setSettingValue).not.toHaveBeenCalled();
  document.removeEventListener('sugarcubes-native-node-card-change', inventory);
});

test('ignores unrelated popup activity and handles palette changes without remounting controls', async () => {
  const { setDirty } = createHost();
  const input = document.createElement('input');
  input.value = 'retained';
  document.body.append(input);
  document.body.append(document.createElement('div'));
  await settle();
  expect(setDirty).toHaveBeenCalledTimes(1);
  document.documentElement.style.colorScheme = 'light';
  await settle();
  expect(setDirty).toHaveBeenCalledTimes(2);
  expect(input.isConnected).toBe(true);
  expect(input.value).toBe('retained');
});

test('identical native stylesheet rewrites do not force palette style reads', async () => {
  const native = document.createElement('style');
  native.textContent = '.native-input { color: white; }';
  document.head.append(native);
  createHost();
  await settle();
  const read = jest.spyOn(window, 'getComputedStyle');
  try {
    for (let index = 0; index < 5; index++) native.textContent = '.native-input { color: white; }';
    await settle();
    expect(read).not.toHaveBeenCalled();
    native.textContent = ':root { --sugarcubes-accent: #ffe066; }';
    await settle();
    expect(read).toHaveBeenCalled();
    read.mockClear();
    native.media = 'print';
    await settle();
    expect(read).toHaveBeenCalled();
  } finally {
    read.mockRestore();
  }
});

test('rejects invalid settings, falls back from invalid CSS, and releases only owned styles', async () => {
  const { setting, report, host } = createHost({ bad: 'data' });
  const style = document.getElementById('sugarcubes-accent-theme')!;
  expect(style.textContent).toContain(`--sc-accent-color:${DEFAULT_ACCENT_COLOR}`);
  setting.onChange('red; } body { display:none');
  expect(report).toHaveBeenCalledWith(expect.any(TypeError));
  document.documentElement.style.setProperty('--sugarcubes-accent', 'transparent');
  await settle();
  expect(style.textContent).toContain(`--sc-accent-color:${DEFAULT_ACCENT_COLOR}`);
  host.dispose();
  expect(style.isConnected).toBe(false);
  expect(document.documentElement.style.getPropertyValue('--sugarcubes-accent')).toBe(
    'transparent',
  );
});

test('uses the negotiated Photoshop accent without persisting it and restores the latest saved choice', () => {
  const embedded = new EmbeddedPresentation();
  const f = createHost('#b58cff', embedded);
  document.documentElement.style.setProperty('--sugarcubes-accent', '#ff0000');
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'dark-gray',
    accentColor: '#d6d6d6',
    resizeCornerSize: 0,
  });
  expect(f.host.theme.color).toBe('#d6d6d6');
  f.setting.onChange('#123456');
  expect(f.host.theme.color).toBe('#d6d6d6');
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'dark-gray',
    accentColor: '#c0c0c0',
    resizeCornerSize: 0,
  });
  expect(f.host.theme.color).toBe('#c0c0c0');
  embedded.set(null);
  expect(f.host.theme.color).toBe('#ff0000');
  document.documentElement.style.removeProperty('--sugarcubes-accent');
  f.host.refresh();
  expect(f.host.theme.color).toBe('#123456');
  expect(f.setSettingValue).not.toHaveBeenCalled();
  f.host.dispose();
  embedded.set({
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'dark-gray',
    accentColor: '#d6d6d6',
    resizeCornerSize: 0,
  });
  expect(f.host.theme.color).toBe('#123456');
});
