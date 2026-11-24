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
/** SugarCubes settings follow the Python SugarSubstitute's pages, in both settings views. */
import { afterEach, expect, test } from '@jest/globals';
import {
  SETTINGS_PAGES,
  sugarCubesSettingsSection,
} from '../../../frontend/comfyui/ui/settings/SugarCubesSettingsSections.js';
import { SettingsPageNavigation } from '../../../frontend/comfyui/ui/settings/SettingsPageNavigation.js';

afterEach(() => document.body.replaceChildren());

test('pages follow the Python app’s order and settings are registered page by page', () => {
  expect(SETTINGS_PAGES.map((page) => page.title)).toEqual([
    'About',
    'Generation',
    'Prompt Editing',
    'Controls',
    'Model Sources',
    'Library',
    'Appearance',
  ]);
  const order = [
    sugarCubesSettingsSection('updates', 'Status'),
    sugarCubesSettingsSection('canvas', 'Visible'),
    sugarCubesSettingsSection('prompts', 'Wildcards'),
    sugarCubesSettingsSection('models', 'Library'),
    sugarCubesSettingsSection('appearance', 'VisualStyle'),
  ];
  for (let index = 1; index < order.length; index++)
    expect(order[index - 1]!.sortOrder).toBeGreaterThan(order[index]!.sortOrder);
  expect(sugarCubesSettingsSection('memory', 'Mode').category).toEqual([
    'SugarCubes',
    'Memory',
    'Mode',
  ]);
});

/** ComfyUI's dialog with its navigation and the SugarCubes category open. */
function dialog(): HTMLElement {
  const root = document.createElement('div');
  root.dataset.testid = 'settings-dialog';
  const nav = document.createElement('nav');
  const application = document.createElement('div');
  application.className = 'flex flex-col gap-2';
  for (const [id, plug] of [
    ['root/Comfy', false],
    ['root/SugarCubes', true],
    ['root/SimpleSyrup', true],
  ] as const) {
    const item = document.createElement('div');
    item.setAttribute('role', 'button');
    item.dataset.navId = id;
    if (plug) {
      const icon = document.createElement('i');
      icon.className = 'icon-[lucide--plug]';
      item.append(icon);
    }
    item.append(id.slice(5));
    application.append(item);
  }
  nav.append(application);
  const content = document.createElement('div');
  for (const [heading, setting] of [
    ['Updates', 'SugarCubes.Updates.Status'],
    ['Output canvas', 'SugarCubes.OutputCanvas.Visible'],
    ['Style', 'SugarCubes.Appearance.VisualStyle'],
  ]) {
    const group = document.createElement('div');
    group.className = 'setting-group';
    const title = document.createElement('h3');
    title.textContent = heading;
    const row = document.createElement('div');
    row.dataset.settingId = setting;
    group.append(title, row);
    content.append(group);
  }
  root.append(nav, content);
  document.body.append(root);
  return root;
}

test('in a plain ComfyUI page the SugarCubes page has its own sub-sidebar, one page at a time', () => {
  const root = dialog();
  const pages = new SettingsPageNavigation(root);
  pages.update(false);
  const bar = root.querySelector('.sc-settings-pages');
  expect([...(bar?.querySelectorAll('button') ?? [])].map((b) => b.textContent)).toEqual([
    'About',
    'Generation',
    'Appearance',
  ]);
  const current = (): string[] =>
    [...root.querySelectorAll('.setting-group[data-sc-settings-page-current]')].map(
      (group) => group.querySelector('h3')?.textContent ?? '',
    );
  expect(current()).toEqual(['Updates']);
  bar?.querySelectorAll('button')[1]?.dispatchEvent(new MouseEvent('click'));
  expect(current()).toEqual(['Output canvas']);
  // ComfyUI's own navigation is untouched outside the frame.
  expect(root.querySelector('.sc-settings-nav-group')).toBeNull();
  expect(root.querySelector('[data-sc-settings-nav-hidden]')).toBeNull();
  pages.dispose();
});

test('in the frame the pages lead the dialog’s navigation and other extensions are grouped', () => {
  const root = dialog();
  const pages = new SettingsPageNavigation(root);
  pages.update(true);
  const nav = root.querySelector('nav');
  const groups = [...(nav?.children ?? [])].map((child) =>
    child.classList.contains('sc-settings-nav-group')
      ? child.querySelector('h3')?.textContent
      : 'comfy',
  );
  expect(groups).toEqual(['SugarSubstitute', 'comfy', 'ComfyUI extensions']);
  expect(root.querySelector('.sc-settings-pages')).toBeNull();
  expect(
    root
      .querySelector('[data-nav-id="root/SugarCubes"]')
      ?.hasAttribute('data-sc-settings-nav-hidden'),
  ).toBe(true);
  expect(
    root
      .querySelector('[data-nav-id="root/SimpleSyrup"]')
      ?.hasAttribute('data-sc-settings-nav-hidden'),
  ).toBe(true);
  expect(
    root.querySelector('[data-nav-id="root/Comfy"]')?.hasAttribute('data-sc-settings-nav-hidden'),
  ).toBe(false);
  const extension = root.querySelector(
    '.sc-settings-nav-group[data-kind="extensions"] [role="button"]',
  );
  expect(extension?.textContent).toBe('SimpleSyrup');
  pages.dispose();
  expect(root.querySelector('.sc-settings-nav-group')).toBeNull();
  expect(root.querySelector('[data-sc-settings-nav-hidden]')).toBeNull();
});
