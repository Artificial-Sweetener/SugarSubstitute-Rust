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
/**
 * Prove the settings workspace presents the Python application's settings: Python's pages,
 * sections and rows as setting cards, the whole dialog in the SugarSubstitute frame with ComfyUI's
 * own settings drawn as the same cards, and one SugarCubes page with segmented tabs in plain
 * ComfyUI. These checks also guard the ComfyUI surfaces the workspace relies on, so a ComfyUI
 * release that changes them fails here by name.
 */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  SettingsWorkspaceHost,
  SETTINGS_ROW_ID,
  describeBinding,
} from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceHost.js';
import { settingsCatalogFor } from '../../../frontend/comfyui/ui/settings/workspace/SettingsCatalog.js';
import { ComfySettingStore } from '../../../frontend/comfyui/ui/settings/workspace/ComfySettingStore.js';
import { SETTINGS_LAYOUT } from '../../../frontend/comfyui/ui/settings/workspace/SettingsLayout.js';
import { settingsLook } from '../../../frontend/comfyui/ui/settings/workspace/SettingsLooks.js';
import { searchSettings } from '../../../frontend/comfyui/ui/settings/workspace/SettingsSearch.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

/** ComfyUI's setting store and settings facade, as extensions see them. */
function comfyApp() {
  const values: Record<string, unknown> = {};
  const definitions: Record<string, Record<string, unknown> & { id: string }> = {};
  const events = new EventTarget();
  const settings = Object.assign(events, {
    addSetting(definition: Record<string, unknown> & { id: string }): void {
      definitions[definition.id] = definition;
    },
  });
  const set = async (id: string, value: unknown): Promise<void> => {
    const old = values[id];
    const onChange = definitions[id]?.onChange;
    if (typeof onChange === 'function') onChange(value, old);
    values[id] = value;
    events.dispatchEvent(new CustomEvent(`${id}.change`, { detail: { value, oldValue: old } }));
  };
  const app = {
    ui: { settings },
    extensionManager: {
      setting: {
        settings: definitions,
        get: (id: string) => (id in values ? values[id] : definitions[id]?.defaultValue),
        set,
      },
      command: { execute: jest.fn() },
    },
  };
  return { app, values, definitions };
}

function host(app: unknown, frame: boolean): SettingsWorkspaceHost {
  return new SettingsWorkspaceHost({
    document,
    app,
    mode: { enabled: frame, desktop: frame, subscribe: () => () => undefined },
    palette: { theme: { color: '#df258e', dark: true }, subscribe: () => () => undefined },
    report: jest.fn(),
  });
}

/** Give one SugarCubes row its switch, bound to a store value. */
function contributeCanvasRow(app: unknown): void {
  const store = new ComfySettingStore(app);
  store.register({
    id: 'SugarCubes.OutputCanvas.Visible',
    name: 'Show output canvas',
    type: 'hidden',
    defaultValue: true,
  });
  settingsCatalogFor(app).contribute('generation.output_canvas.visible', {
    controls: [
      { kind: 'switch', value: store.value('SugarCubes.OutputCanvas.Visible', (v) => v !== false) },
    ],
  });
}

const settle = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

test('in plain ComfyUI one SugarCubes row carries the workspace: segmented tabs over Python pages', async () => {
  const { app, definitions } = comfyApp();
  contributeCanvasRow(app);
  const workspace = host(app, false);
  workspace.setup();
  const row = definitions[SETTINGS_ROW_ID];
  if (!row || typeof row.type !== 'function') throw new Error('Missing workspace row');
  const element = (row.type as () => unknown)();
  if (!(element instanceof HTMLElement)) throw new Error('The workspace row did not render');
  document.body.append(element);
  // ComfyUI calls a custom renderer again on every re-render: the same workspace comes back.
  expect((row.type as () => unknown)()).toBe(element);
  expect(element.dataset.mode).toBe('page');
  const tabs = [
    ...element.querySelectorAll<HTMLButtonElement>('.sc-segmented-slider [role="tab"]'),
  ];
  expect(tabs.map((tab) => tab.textContent)).toEqual(['About', 'Generation', 'Controls']);
  expect(element.querySelector('.sc-segmented-slider-indicator')).not.toBeNull();
  tabs[1]?.click();
  await settle();
  expect(
    [...element.querySelectorAll('.sc-sw-section:not([hidden]) .sc-sw-section-title')].map(
      (title) => title.textContent,
    ),
  ).toEqual(['Output canvas']);
  const card = element.querySelector<HTMLElement>('[data-row="generation.output_canvas.visible"]');
  expect(card?.querySelector('.sc-sw-card-title')?.textContent).toBe('Show output canvas');
  expect(card?.querySelector('.sc-sw-card-description')?.textContent).toContain('output canvas');
  // Nothing of ComfyUI's own dialog is touched: the workspace lives inside its one row.
  expect(document.querySelector('[data-sc-sw-host]')).toBeNull();
  workspace.dispose();
});

test('in the frame the workspace is the dialog, with ComfyUI’s settings as the same cards', async () => {
  const { app, values } = comfyApp();
  contributeCanvasRow(app);
  const store = new ComfySettingStore(app);
  store.register({
    id: 'Comfy.Graph.Snap',
    name: 'Snap to grid',
    type: 'boolean',
    tooltip: 'Align nodes to the grid.',
    category: ['Comfy', 'Graph', 'Snap'],
    defaultValue: false,
  });
  store.register({
    id: 'Syrup.Mode',
    name: 'Syrup mode',
    type: 'combo',
    options: ['thin', 'thick'],
    category: ['SimpleSyrup', 'Mode'],
    defaultValue: 'thin',
  });
  const root = document.createElement('div');
  root.dataset.testid = 'settings-dialog';
  const grid = document.createElement('div');
  grid.className = 'grid';
  const nav = document.createElement('nav');
  const clicked: string[] = [];
  for (const [id, label] of [
    ['root/Comfy', 'Comfy'],
    ['root/SugarCubes', 'SugarCubes'],
    ['root/SimpleSyrup', 'SimpleSyrup'],
    ['keybinding', 'Keybinding'],
    ['about', 'About'],
  ]) {
    const item = document.createElement('div');
    item.dataset.navId = id;
    item.append(document.createElement('i'), label ?? '');
    item.addEventListener('click', () => clicked.push(id ?? ''));
    nav.append(item);
  }
  grid.append(nav);
  root.append(grid);
  document.body.append(root);
  const workspace = host(app, true);
  workspace.setup();
  await settle();
  const view = root.querySelector<HTMLElement>('.sc-sw[data-mode="frame"]');
  if (!view) throw new Error('The frame workspace did not mount');
  expect(root.hasAttribute('data-sc-sw-host')).toBe(true);
  const entries = [...view.querySelectorAll('.sc-sw-nav > *')].map((item) =>
    item.classList.contains('sc-sw-nav-group')
      ? `# ${item.textContent ?? ''}`
      : (item.textContent ?? ''),
  );
  expect(entries.filter((entry) => entry !== '')).toEqual([
    '# Sugar',
    'About',
    'Generation',
    'Controls',
    '# ComfyUI',
    'Comfy',
    'Keybinding',
    'About',
    '# ComfyUI extensions',
    'SimpleSyrup',
  ]);
  expect(view.querySelector('.sc-sw-header')?.textContent).toBe('Settings › About');
  // Our About and ComfyUI's About panel are distinct entries: only ours is the open page.
  const current = [...view.querySelectorAll<HTMLElement>('.sc-sw-nav [aria-current="page"]')];
  expect(current.map((item) => item.dataset.entry)).toEqual(['about']);
  view.querySelector<HTMLElement>('[data-entry="root/Comfy"]')?.click();
  await settle();
  expect(view.querySelector('.sc-sw-header')?.textContent).toBe('Settings › Comfy');
  const snap = view.querySelector<HTMLElement>('[data-row="Comfy.Graph.Snap"]');
  expect(snap?.querySelector('.sc-sw-card-description')?.textContent).toBe(
    'Align nodes to the grid.',
  );
  snap?.querySelector<HTMLElement>('[role="switch"]')?.click();
  await settle();
  expect(values['Comfy.Graph.Snap']).toBe(true);
  // ComfyUI's own panels show through: the workspace selects ComfyUI's entry for them.
  view.querySelector<HTMLElement>('[data-entry="panel/keybinding"]')?.click();
  await settle();
  expect(root.getAttribute('data-sc-sw-panel')).toBe('keybinding');
  expect(clicked).toEqual(['keybinding']);
  workspace.dispose();
  expect(root.hasAttribute('data-sc-sw-host')).toBe(false);
  expect(root.querySelector('.sc-sw')).toBeNull();
});

test('every style draws the same workspace in its own look; Fluent in the frame is Python’s', () => {
  expect(settingsLook('fluent', true, 'frame')).toMatchObject({
    card: 'rgb(43,43,43)',
    radius: '6px',
    text: '#fff',
  });
  expect(settingsLook('fluent', false, 'frame')).toMatchObject({
    card: 'rgba(255,255,255,.702)',
    text: '#000',
  });
  expect(settingsLook('fluent', true, 'page').card).toBe('var(--sc-surface-secondary)');
  expect(settingsLook('photopro', true, 'frame').radius).toBe('0');
  expect(settingsLook('comfy', true, 'frame').text).toBe('var(--fg-color,#ddd)');
});

test('every row has a place and Python’s text before a feature can fill it', () => {
  const ids = SETTINGS_LAYOUT.flatMap((page) =>
    page.sections.flatMap((section) => section.rows.map((row) => row.id)),
  );
  expect(new Set(ids).size).toBe(ids.length);
  expect(SETTINGS_LAYOUT.map((page) => page.title)).toEqual([
    'About',
    'Generation',
    'Prompt Editing',
    'Controls',
    'Model Sources',
    'Library',
    'ComfyUI',
    'Appearance',
  ]);
  expect(() => settingsCatalogFor({}).contribute('generation.unplaced', {})).toThrow(
    'has no place',
  );
});

test('search follows Python: every word starts a word of the row, its section or its page', () => {
  const { app } = comfyApp();
  contributeCanvasRow(app);
  const pages = settingsCatalogFor(app).pages();
  expect(searchSettings(pages, 'outp canv').map((result) => result.row.id)).toEqual([
    'generation.output_canvas.visible',
  ]);
  expect(searchSettings(pages, 'anvas')).toEqual([]);
});

test('a ComfyUI release that changes the settings surface is named', () => {
  expect(() => new ComfySettingStore({}).get('Comfy.X')).toThrow('ComfyUI settings changed');
  expect(() => new ComfySettingStore({}).subscribe('Comfy.X', () => undefined)).toThrow(
    'ComfyUI settings changed',
  );
  expect(() => new ComfySettingStore({}).register({ id: 'X' })).toThrow('ComfyUI settings changed');
});

test('a command’s bound key reads as Python wrote shortcuts', () => {
  const bindings = [
    { commandId: 'SugarCubes.ToggleConsole', combo: { key: 'k', ctrl: true, shift: true } },
  ];
  expect(describeBinding(bindings, 'SugarCubes.ToggleConsole')).toBe('Ctrl+Shift+K');
  expect(describeBinding(bindings, 'SugarCubes.ToggleMasonry')).toBe('Not assigned');
  expect(describeBinding(null, 'SugarCubes.ToggleMasonry')).toBe('Not assigned');
  expect(isRecord(bindings[0])).toBe(true);
});
