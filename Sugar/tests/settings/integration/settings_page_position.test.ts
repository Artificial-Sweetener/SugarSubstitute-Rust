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
/** Verify settings pages keep independent reading positions without rebuilding their controls. */
import { afterEach, expect, test } from '@jest/globals';
import { SettingsPagePosition } from '../../../frontend/comfyui/ui/settings/SettingsPagePosition.js';

afterEach(() => document.body.replaceChildren());

test('start a fresh page at the top and restore the prior page offset on return', () => {
  const root = document.createElement('section');
  const nav = document.createElement('nav');
  const main = document.createElement('main');
  const scroller = document.createElement('div');
  main.append(scroller);
  root.append(nav, main);
  document.body.append(root);
  const entries = ['Comfy', 'Plugin'].map((id) => {
    const entry = document.createElement('button');
    entry.dataset.navId = id;
    entry.addEventListener('click', () => {
      for (const button of nav.querySelectorAll('button')) button.className = '';
      entry.className = 'bg-interface-menu-component-surface-selected';
    });
    nav.append(entry);
    return entry;
  });
  const [comfy, plugin] = entries;
  if (!comfy || !plugin) throw new Error('Missing native navigation.');
  const position = new SettingsPagePosition(root);
  comfy.click();
  position.reconcile();
  scroller.scrollTop = 350;
  scroller.dispatchEvent(new Event('scroll'));
  plugin.click();
  position.reconcile();
  expect(scroller.scrollTop).toBe(0);
  scroller.scrollTop = 120;
  scroller.dispatchEvent(new Event('scroll'));
  comfy.click();
  position.reconcile();
  expect(scroller.scrollTop).toBe(350);
  plugin.click();
  position.reconcile();
  expect(scroller.scrollTop).toBe(120);
  position.dispose();
});
