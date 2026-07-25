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
/** Verify Comfy sidebar registration stays thin and idempotent. */

import { jest } from '@jest/globals';
import {
  SugarCubesSidebarHostAdapter,
  type SugarCubesSidebarManager,
} from '../../frontend/comfyui/ui/sidebar/SugarCubesSidebarHostAdapter.js';

test('registers once and reuses one browser mount across host renders', () => {
  const registerSidebarTab = jest.fn();
  let registration: Parameters<SugarCubesSidebarManager['registerSidebarTab']>[0] | undefined;
  const manager: SugarCubesSidebarManager = {
    registerSidebarTab(tab) {
      registration = tab;
      registerSidebarTab(tab);
    },
  };
  const mountEmbedded = jest.fn();
  const adapter = new SugarCubesSidebarHostAdapter({
    document,
    getManager: () => manager,
    browser: { mountEmbedded },
    logger: console,
  });

  adapter.register();
  adapter.register();
  expect(registerSidebarTab).toHaveBeenCalledTimes(1);
  if (!registration) throw new Error('Expected sidebar registration.');
  const firstHost = document.createElement('div');
  const secondHost = document.createElement('div');
  registration.render(firstHost);
  registration.render(secondHost);

  expect(mountEmbedded).toHaveBeenCalledTimes(1);
  expect(firstHost.childElementCount).toBe(0);
  expect(secondHost.querySelector('.sugarcubes-sidebar-panel__browser')).not.toBeNull();
  registration.destroy();
  expect(secondHost.childElementCount).toBe(0);
});
