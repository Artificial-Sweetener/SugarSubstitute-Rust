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
  ACCENT_SOURCE_SETTING,
} from '../../../frontend/comfyui/ui/surface/appearance/AccentThemeHost.js';
import {
  DEFAULT_ACCENT_COLOR,
  createAccentPalette,
} from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

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
import { HostAccent } from '../../../frontend/comfyui/ui/core/embedded/HostAccent.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import { DeclaredHost } from '../../../frontend/comfyui/ui/core/embedded/PanelRuntime.js';

const hosts: AccentThemeHost[] = [];
afterEach(() => {
  hosts.splice(0).forEach((host) => host.dispose());
  document.documentElement.removeAttribute('style');
  document.documentElement.removeAttribute('data-sc-visual-style');
  document.documentElement.removeAttribute('data-sc-photopro-palette');
  document.head.replaceChildren();
  document.body.replaceChildren();
});

/** Settle a host that declares its accent is the system accent. */
function systemAccentHost(): HostDescriptionState {
  const declared = DeclaredHost.read({
    protocols: [{ name: 'sugarcubes.panel', version: 3 }],
    capabilities: [{ name: 'presentation.system-accent' }],
  });
  if (!declared) throw new Error('The fixture description was refused.');
  const state = new HostDescriptionState();
  state.settle(declared);
  return state;
}

/** Bind a real observer and stylesheet to a narrow Comfy settings boundary. */
function createHost(
  saved: unknown = '#b58cff',
  embedded?: EmbeddedPresentationSource,
  hostAccent?: HostAccent,
  source: unknown = 'system',
  description: HostDescriptionState | undefined = hostAccent ? systemAccentHost() : undefined,
) {
  let setting: ColorSetting | undefined;
  let sourceSetting: ColorSetting | undefined;
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
          getSettingValue: (id: string) => (id === ACCENT_SOURCE_SETTING ? source : saved),
          addSetting: (value: ColorSetting) => {
            if (value.id === ACCENT_SOURCE_SETTING) sourceSetting = value;
            else setting = value;
          },
          setSettingValue,
        },
      },
    },
    report,
    embedded,
    hostAccent,
    description,
  );
  hosts.push(host);
  host.setup();
  return {
    host,
    setting: setting!,
    get sourceSetting() {
      return sourceSetting;
    },
    setDirty,
    report,
    setSettingValue,
  };
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
  expect(style.textContent).toContain('--sc-accent-color:#c19eff');
  const row = setting.type();
  const input = row.querySelector<HTMLInputElement>('input[type="text"]')!;
  input.value = '#071530';
  input.dispatchEvent(new Event('change'));
  expect(setSettingValue).toHaveBeenCalledWith(ACCENT_COLOR_SETTING, '#071530');
  expect(style.textContent).toContain('--sc-accent-foreground:#000000');
  expect(style.textContent).toContain('--sc-accent-color:#4887ff');
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
  expect(style.textContent).toContain('--sc-accent-color:#ffe57e');
  expect(style.textContent).toContain('--sc-accent-foreground:#000000');
  setting.onChange('#123456');
  expect(style.textContent).toContain('--sc-accent-color:#ffe57e');
  document.documentElement.style.removeProperty('--sugarcubes-accent');
  await settle();
  expect(style.textContent).toContain('--sc-accent-color:#56aaff');
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
  expect(style.textContent).toContain(
    `--sc-accent-color:${createAccentPalette(DEFAULT_ACCENT_COLOR, true).indicator}`,
  );
  setting.onChange('red; } body { display:none');
  expect(report).toHaveBeenCalledWith(expect.any(TypeError));
  document.documentElement.style.setProperty('--sugarcubes-accent', 'transparent');
  await settle();
  expect(style.textContent).toContain(
    `--sc-accent-color:${createAccentPalette(DEFAULT_ACCENT_COLOR, true).indicator}`,
  );
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

test('PhotoPro brightness follows accepted palette changes independently of Comfy theme and accent', async () => {
  document.documentElement.style.setProperty('--comfy-menu-bg', '#ffffff');
  document.documentElement.dataset.scVisualStyle = 'photopro';
  document.documentElement.dataset.scPhotoproPalette = 'medium-gray';
  const { host, setSettingValue } = createHost('#d6d6d6');
  const changed = jest.fn();
  host.subscribe(changed);
  expect(host.theme).toEqual({ color: '#d6d6d6', dark: true });
  document.documentElement.dataset.scPhotoproPalette = 'light-gray';
  await settle();
  expect(host.theme).toEqual({ color: '#d6d6d6', dark: false });
  document.documentElement.dataset.scPhotoproPalette = 'dark-gray';
  await settle();
  expect(host.theme).toEqual({ color: '#d6d6d6', dark: true });
  document.documentElement.style.setProperty('--comfy-menu-bg', '#202020');
  await settle();
  expect(changed).toHaveBeenCalledTimes(2);
  expect(setSettingValue).not.toHaveBeenCalled();
});

test('keeps the connected host accent while preserving the saved browser choice', () => {
  const hostAccent = new HostAccent();
  hostAccent.set('#0078d4');
  const f = createHost(null, undefined, hostAccent);
  expect(f.host.theme.color).toBe('#0078d4');
  hostAccent.set('#107c10');
  expect(f.host.theme.color).toBe('#107c10');
  hostAccent.set(null);
  expect(f.host.theme.color).toBe(DEFAULT_ACCENT_COLOR);
  hostAccent.set('#0078d4');
  f.setting.onChange('#123456');
  expect(f.host.theme.color).toBe('#0078d4');
  hostAccent.set('#107c10');
  expect(f.host.theme.color).toBe('#107c10');
  hostAccent.set(null);
  expect(f.host.theme.color).toBe('#123456');
  expect(f.setSettingValue).not.toHaveBeenCalled();
});

test('the host accent outranks a persisted default or custom browser accent without rewriting either', () => {
  const hostAccent = new HostAccent();
  hostAccent.set('#0078d4');
  let setting: ColorSetting | undefined;
  const host = new AccentThemeHost(
    document,
    {
      ui: {
        settings: {
          getSettingValue: () => undefined,
          addSetting: (value: ColorSetting) => {
            if (value.id === ACCENT_COLOR_SETTING) setting = value;
            // Comfy announces the registered default synchronously during registration.
            value.onChange(DEFAULT_ACCENT_COLOR);
          },
          setSettingValue: jest.fn(),
        },
      },
    },
    jest.fn(),
    undefined,
    hostAccent,
    systemAccentHost(),
  );
  hosts.push(host);
  host.setup();
  expect(host.theme.color).toBe('#0078d4');
  setting!.onChange(DEFAULT_ACCENT_COLOR);
  expect(host.theme.color).toBe('#0078d4');
  const saved = createHost('#b58cff', undefined, hostAccent);
  const persistedDefault = createHost(DEFAULT_ACCENT_COLOR, undefined, hostAccent);
  expect(saved.host.theme.color).toBe('#0078d4');
  expect(persistedDefault.host.theme.color).toBe('#0078d4');
  hostAccent.set('#d65a36');
  expect(saved.host.theme.color).toBe('#d65a36');
  expect(persistedDefault.host.theme.color).toBe('#d65a36');
  hostAccent.set(null);
  expect(saved.host.theme.color).toBe('#b58cff');
  expect(persistedDefault.host.theme.color).toBe(DEFAULT_ACCENT_COLOR);
  expect(saved.setSettingValue).not.toHaveBeenCalled();
  expect(persistedDefault.setSettingValue).not.toHaveBeenCalled();
});

test('a system-accent host defaults to it and switches live between System and Custom without losing either', () => {
  const accent = new HostAccent();
  accent.set('#c30052');
  const f = createHost('#479ef5', undefined, accent);
  expect(f.sourceSetting?.id).toBe(ACCENT_SOURCE_SETTING);
  expect(f.host.theme.color).toBe('#c30052');
  const picker = f.setting.type() as HTMLFieldSetElement;
  expect(picker.disabled).toBe(true);
  f.sourceSetting!.onChange('custom');
  expect(f.host.theme.color).toBe('#479ef5');
  expect(picker.disabled).toBe(false);
  f.setting.onChange('#e91e63');
  expect(f.host.theme.color).toBe('#e91e63');
  accent.set('#107c10');
  expect(f.host.theme.color).toBe('#e91e63');
  f.sourceSetting!.onChange('system');
  expect(f.host.theme.color).toBe('#107c10');
  expect(picker.disabled).toBe(true);
  expect(picker.querySelector('input')?.value).toBe('#107c10');
  accent.set(null);
  expect(f.host.theme.color).toBe('#e91e63');
  expect(picker.disabled).toBe(false);
  expect(f.setSettingValue).not.toHaveBeenCalled();
});

test('restores a persisted Custom source under a system-accent host and ignores invalid source changes', () => {
  const accent = new HostAccent();
  accent.set('#c30052');
  const f = createHost('#b58cff', undefined, accent, 'custom');
  expect(f.host.theme.color).toBe('#b58cff');
  f.sourceSetting!.onChange('invalid');
  expect(f.host.theme.color).toBe('#b58cff');
  expect(f.report).toHaveBeenCalledTimes(1);
});

test('without a declared system accent the source setting is absent and the saved or default color applies', () => {
  const accent = new HostAccent();
  accent.set('#c30052');
  const saved = createHost('#b58cff', undefined, accent, 'system', new HostDescriptionState());
  const defaults = createHost(null, undefined, accent, 'system', new HostDescriptionState());
  expect(saved.sourceSetting).toBeUndefined();
  expect(defaults.sourceSetting).toBeUndefined();
  expect(saved.host.theme.color).toBe('#b58cff');
  expect(defaults.host.theme.color).toBe(DEFAULT_ACCENT_COLOR);
  expect((saved.setting.type() as HTMLFieldSetElement).disabled).toBe(false);
  saved.setting.onChange('#e91e63');
  expect(saved.host.theme.color).toBe('#e91e63');
});

test('recomputes all variant roles when the mode changes without changing the base', async () => {
  document.documentElement.style.colorScheme = 'dark';
  const { host, setSettingValue } = createHost('#0078d4');
  const style = document.getElementById('sugarcubes-accent-theme')!;
  expect(style.textContent).toContain('--sc-accent-color:#29a2ff');
  expect(style.textContent).toContain('--sc-accent-hover:#2592e6');
  document.documentElement.style.colorScheme = 'light';
  await settle();
  expect(host.theme).toEqual({ color: '#0078d4', dark: false });
  expect(style.textContent).toContain('--sc-accent-color:#0078d4');
  expect(style.textContent).toContain('--sc-accent-hover:#007edf');
  expect(style.textContent).toContain('--sc-accent-pressed:#4ea0df');
  expect(style.textContent).toContain('--sc-accent-toggle-pressed:#3896df');
  expect(setSettingValue).not.toHaveBeenCalled();
});

test('offers the system accent source when the declaration settles after setup', () => {
  const accent = new HostAccent();
  accent.set('#c30052');
  const pending = new HostDescriptionState();
  const f = createHost('#479ef5', undefined, accent, 'system', pending);
  expect(f.sourceSetting).toBeUndefined();
  expect(f.host.theme.color).toBe('#479ef5');
  const declared = DeclaredHost.read({
    protocols: [{ name: 'sugarcubes.panel', version: 3 }],
    capabilities: [{ name: 'presentation.system-accent' }],
  });
  if (!declared) throw new Error('The fixture description was refused.');
  pending.settle(declared);
  expect(f.sourceSetting?.id).toBe(ACCENT_SOURCE_SETTING);
  expect(f.host.theme.color).toBe('#c30052');
  expect(f.setSettingValue).not.toHaveBeenCalled();
  expect(f.report).not.toHaveBeenCalled();
});
