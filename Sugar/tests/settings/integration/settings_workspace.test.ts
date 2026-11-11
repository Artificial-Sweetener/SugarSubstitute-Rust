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
/** Safeguard native settings identity and the isolation of bespoke plugin controls. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ComfySettingsCatalog } from '../../../frontend/comfyui/ui/settings/ComfySettingsCatalog.js';
import { SettingsWorkspaceSurface } from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceSurface.js';
import { SettingsWorkspaceHost } from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceHost.js';
import { SettingsPopupTheme } from '../../../frontend/comfyui/ui/settings/SettingsPopupTheme.js';
import { settingsWorkspaceCss } from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceStyles.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';
import { Switch } from '@spectrum-web-components/switch/src/Switch.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  jest.restoreAllMocks();
  document.documentElement.removeAttribute('data-sc-visual-style');
});

test('Spectrum settings retain the native value and persistence owner while switching providers reversibly', async () => {
  const f = fixture();
  const row = f.rows[0];
  const input = row?.querySelector('input');
  if (!row || !input) throw new Error('Missing standard setting');
  input.type = 'checkbox';
  input.setAttribute('aria-label', '<img src=x> Enabled');
  const native = document.createElement('div');
  native.dataset.pcName = 'toggleswitch';
  input.before(native);
  native.append(input);
  const persisted = jest.fn();
  input.addEventListener('change', persisted);
  const surface = new SettingsWorkspaceSurface(f.root, f.catalog);
  publishVisualStyle(document, 'spectrum');
  surface.update(true, true);
  const control = row.querySelector('[data-sc-settings-switch]');
  if (!(control instanceof Switch)) throw new Error('Expected an actual Spectrum switch');
  await control.updateComplete;
  expect(native.hidden).toBe(true);
  expect(row.querySelector('img')).toBeNull();
  expect(control.focusElement.getAttribute('aria-label')).toBe('<img src=x> Enabled');
  control.click();
  await control.updateComplete;
  expect(input.checked).toBe(true);
  expect(persisted).toHaveBeenCalledTimes(1);
  input.checked = false;
  input.dispatchEvent(new Event('change'));
  await control.updateComplete;
  expect(control.checked).toBe(false);
  input.disabled = true;
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await control.updateComplete;
  expect(control.disabled).toBe(true);
  control.click();
  expect(persisted).toHaveBeenCalledTimes(2);
  input.disabled = false;
  surface.update(true, false);
  await control.updateComplete;
  expect(row.querySelector('[data-sc-settings-switch]')).toBe(control);
  control.focus();
  await control.updateComplete;
  publishVisualStyle(document, 'fluent');
  surface.update(true, false);
  expect(row.querySelector('[data-sc-settings-switch]')).toBeNull();
  expect(native.hidden).toBe(false);
  expect(row.querySelector('input')).toBe(input);
  expect(document.activeElement).toBe(input);
  expect(persisted).toHaveBeenCalledTimes(2);
  surface.dispose();
});

/** Build the observed host boundaries with an independently owned plugin page. */
function fixture() {
  const root = document.createElement('div');
  root.dataset.testid = 'settings-dialog';
  const grid = document.createElement('div');
  const sidebar = document.createElement('nav');
  const title = document.createElement('header');
  title.textContent = 'Settings';
  const searchSlot = document.createElement('div');
  const input = document.createElement('input');
  input.placeholder = 'Search Settings...';
  searchSlot.append(input);
  sidebar.append(title, searchSlot, document.createElement('nav'));
  const content = document.createElement('div');
  const header = document.createElement('header');
  const close = document.createElement('button');
  close.setAttribute('aria-label', 'Close dialog');
  header.append(close);
  const main = document.createElement('main');
  const rows = ['Plugin.Enabled', 'Plugin.Custom', 'SugarCubes.Output.Files'].map((id) => {
    const row = document.createElement('div');
    row.dataset.settingId = id;
    const label = document.createElement('div');
    label.className = 'form-label';
    const control = document.createElement('input');
    row.append(label, control);
    return row;
  });
  main.append(...rows);
  content.append(header, main);
  grid.append(sidebar, content);
  root.append(grid);
  document.body.append(root);
  const custom = jest.fn();
  const catalog = new ComfySettingsCatalog({
    ui: {
      settings: {
        settingsParamLookup: {
          'Plugin.Enabled': {
            id: 'Plugin.Enabled',
            name: 'Enabled',
            type: 'boolean',
            tooltip: '<b>Literal</b>',
          },
          'Plugin.Custom': { id: 'Plugin.Custom', name: 'Custom', type: custom },
          'SugarCubes.Output.Files': { id: 'SugarCubes.Output.Files', name: 'Files', type: custom },
        },
      },
    },
  });
  return { root, sidebar, searchSlot, input, close, header, rows, custom, catalog };
}

test('move the original search and close controls and restore their exact slots on mode exit', () => {
  const f = fixture();
  const changed = jest.fn();
  f.input.addEventListener('input', changed);
  f.input.value = 'focused search';
  f.input.focus();
  f.input.setSelectionRange(2, 5);
  const surface = new SettingsWorkspaceSurface(f.root, f.catalog);
  surface.update(true, true);
  expect(f.input.closest('.sc-settings-search-bar')).not.toBeNull();
  expect(document.activeElement).toBe(f.input);
  expect(f.input.selectionStart).toBe(2);
  expect(f.close.closest('.sc-settings-search-bar')).not.toBeNull();
  f.input.value = 'existing search';
  f.input.dispatchEvent(new Event('input', { bubbles: true }));
  surface.update(true, false);
  expect(changed).toHaveBeenCalledTimes(1);
  expect(f.input.value).toBe('existing search');
  surface.update(false, false);
  expect(f.searchSlot.parentElement).toBe(f.sidebar);
  expect(f.close.parentElement).toBe(f.header);
  expect(f.root.querySelector('.sc-settings-search-bar')).toBeNull();
  expect(document.activeElement).toBe(f.input);
  surface.dispose();
});

test('retains native sidebar collapse authority and both header actions in themed settings', () => {
  const f = fixture();
  const grid = f.root.firstElementChild;
  if (!(grid instanceof HTMLElement)) throw new Error('Missing native settings grid.');
  const toggle = document.createElement('button');
  toggle.setAttribute('aria-label', 'Show left panel');
  f.header.prepend(toggle);
  f.sidebar.inert = true;
  f.sidebar.setAttribute('aria-hidden', 'true');
  grid.style.gridTemplateColumns = '0rem 1fr';
  toggle.addEventListener('click', () => {
    f.sidebar.inert = !f.sidebar.inert;
    f.sidebar.setAttribute('aria-hidden', String(f.sidebar.inert));
    grid.style.gridTemplateColumns = f.sidebar.inert ? '0rem 1fr' : '18rem 1fr';
  });
  const closed = jest.fn();
  f.close.addEventListener('click', closed);
  const style = document.createElement('style');
  style.textContent = settingsWorkspaceCss;
  document.head.append(style);
  const surface = new SettingsWorkspaceSurface(f.root, f.catalog);
  surface.update(true, true);
  expect(f.sidebar.inert).toBe(true);
  expect(getComputedStyle(grid).gridTemplateColumns).toBe('0rem 1fr');
  expect(toggle.closest('.sc-settings-search-bar')).not.toBeNull();
  expect(f.close.closest('.sc-settings-search-bar')).not.toBeNull();
  toggle.click();
  expect(f.sidebar.inert).toBe(false);
  expect(getComputedStyle(grid).gridTemplateColumns).toBe('18rem 1fr');
  f.close.click();
  expect(closed).toHaveBeenCalledTimes(1);
  toggle.click();
  expect(f.sidebar.inert).toBe(true);
  surface.dispose();
  expect([...f.header.children]).toEqual([toggle, f.close]);
  expect(f.sidebar.inert).toBe(true);
});

test('theme registered standard rows while preserving custom plugin subtrees and callbacks', () => {
  const f = fixture();
  const customTree = f.rows[1]?.cloneNode(true);
  const surface = new SettingsWorkspaceSurface(f.root, f.catalog);
  surface.update(true, true);
  expect(f.rows[0]?.dataset.scSettingKind).toBe('standard');
  expect(f.rows[0]?.querySelector('.sc-setting-description')?.textContent).toBe('<b>Literal</b>');
  expect(f.rows[0]?.querySelector('b')).toBeNull();
  expect(f.rows[1]?.isEqualNode(customTree ?? null)).toBe(true);
  expect(f.rows[2]?.dataset.scSettingKind).toBe('owned');
  surface.update(true, true);
  expect(f.rows[0]?.querySelectorAll('.sc-setting-description')).toHaveLength(1);
  expect(f.custom).not.toHaveBeenCalled();
  surface.dispose();
  expect(f.rows[0]?.hasAttribute('data-sc-setting-kind')).toBe(false);
});

test('ignore unrelated portals and preserve mounted settings controls across mode changes', async () => {
  const f = fixture();
  const modeListeners = new Set<() => void>();
  const mode = {
    enabled: true,
    subscribe: (callback: () => void) => {
      modeListeners.add(callback);
      return () => modeListeners.delete(callback);
    },
  };
  const register = jest.fn();
  const host = new SettingsWorkspaceHost({
    document,
    app: { ui: { settings: { addSetting: register, settingsParamLookup: {} } } },
    mode,
    palette: { theme: { dark: true, color: '#479ef5' }, subscribe: () => () => {} },
    report: (error) => {
      throw error;
    },
  });
  const reads = jest.spyOn(ComfySettingsCatalog.prototype, 'find');
  host.setup();
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  reads.mockClear();
  const portal = document.createElement('div');
  portal.setAttribute('role', 'dialog');
  portal.className = 'p-select-overlay';
  portal.append(document.createElement('input'));
  document.body.append(portal);
  portal.append(document.createElement('button'));
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(reads).not.toHaveBeenCalled();
  mode.enabled = false;
  for (const listener of modeListeners) listener();
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(f.searchSlot.parentElement).toBe(f.sidebar);
  expect(register).toHaveBeenCalledTimes(1);
  host.dispose();
  expect(modeListeners.size).toBe(0);
  expect(f.root.hasAttribute('data-sc-settings')).toBe(false);
});

test('return to the prior native category after clearing search', () => {
  const f = fixture();
  const navigation = f.sidebar.querySelector('nav');
  const entries = ['Comfy', 'Plugin'].map((id) => {
    const button = document.createElement('button');
    button.dataset.navId = id;
    navigation?.append(button);
    return button;
  });
  const [comfy, plugin] = entries;
  if (!comfy || !plugin) throw new Error('Missing navigation fixture.');
  plugin.className = 'bg-interface-menu-component-surface-selected';
  const restore = jest.fn();
  plugin.addEventListener('click', restore);
  const surface = new SettingsWorkspaceSurface(f.root, f.catalog);
  surface.update(true, true);
  f.input.value = 'preview';
  f.input.dispatchEvent(new Event('input', { bubbles: true }));
  plugin.className = '';
  surface.update(true, true);
  expect(restore).not.toHaveBeenCalled();
  f.input.value = '';
  comfy.className = 'bg-interface-menu-component-surface-selected';
  surface.update(true, true);
  surface.update(true, true);
  expect(restore).toHaveBeenCalledTimes(1);
  surface.dispose();
});

test('theme only portals anchored to standard settings and restore their host tokens on exit', () => {
  const f = fixture();
  const surface = new SettingsWorkspaceSurface(f.root, f.catalog);
  surface.update(true, true);
  const control = f.rows[0]?.querySelector('input');
  control?.setAttribute('aria-controls', 'standard-options');
  const overlays = ['standard-options', 'plugin-options'].map((id) => {
    const overlay = document.createElement('div');
    const list = document.createElement('ul');
    list.id = id;
    list.setAttribute('role', 'listbox');
    overlay.append(list);
    document.body.append(overlay);
    return overlay;
  });
  const [standard, plugin] = overlays;
  if (!standard || !plugin) throw new Error('Missing overlay fixture.');
  const theme = new SettingsPopupTheme();
  theme.reconcile(standard, [f.root], true, true);
  theme.reconcile(plugin, [f.root], true, true);
  expect(standard.hasAttribute('data-sc-settings-popup')).toBe(true);
  expect(plugin.hasAttribute('data-sc-settings-popup')).toBe(false);
  theme.refresh([f.root], false, true);
  expect(standard.hasAttribute('data-sc-settings-popup')).toBe(false);
  theme.refresh([f.root], true, false);
  expect(standard.hasAttribute('data-sc-settings-popup')).toBe(true);
  expect(plugin.hasAttribute('data-sc-settings-popup')).toBe(false);
  theme.dispose();
  surface.dispose();
});
