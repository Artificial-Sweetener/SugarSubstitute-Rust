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
import { describe, expect, test, jest } from '@jest/globals';
import { CubeBrowserController } from '../../web/comfyui/ui/browser/CubeBrowserController.js';
import type {
  BrowserActions,
  BrowserApi,
} from '../../web/comfyui/ui/browser/CubeBrowserController.js';

function createDeleteApi() {
  return {
    delete: jest.fn<BrowserApi['delete']>().mockResolvedValue({ response: { ok: true }, data: {} }),
  };
}

function createConfirmAction(confirmed: boolean) {
  return jest.fn<NonNullable<BrowserActions['openConfirmDialog']>>(async () => confirmed);
}

describe('cube browser delete', () => {
  test('delete uses cube id when available', async () => {
    const api = createDeleteApi();
    const controller = new CubeBrowserController({ api });
    controller.actions = {
      openConfirmDialog: createConfirmAction(true),
    };
    controller.toast = { push: jest.fn() };
    controller.render = jest.fn();
    controller.removeCubeFromLists = jest.fn();

    const cube = {
      name: 'automask_detailer_fork',
      cube_id: 'local/example-user/automask_detailer_fork.cube',
      is_writable: true,
      relative_path: '_forks/test/automask_detailer_fork.cube',
      supported_models: [],
      tags: [],
      mtime: '',
    };
    controller.store.setCubes([cube]);
    controller.store.setFiltered([cube]);
    controller.store.setSelected(cube.cube_id);

    await controller.requestDelete();

    expect(api.delete).toHaveBeenCalledWith({
      cube_id: 'local/example-user/automask_detailer_fork.cube',
    });
    expect(controller.actions.openConfirmDialog).toHaveBeenCalledWith({
      title: 'Delete SugarCube?',
      message: ['Delete SugarCube "automask_detailer_fork"?', 'This cannot be undone.'],
      confirmLabel: 'Delete',
    });
  });

  test('delete rejects cubes without canonical ids', async () => {
    const api = createDeleteApi();
    const controller = new CubeBrowserController({ api });
    controller.actions = {
      openConfirmDialog: createConfirmAction(true),
    };
    controller.toast = { push: jest.fn() };
    controller.render = jest.fn();
    controller.removeCubeFromLists = jest.fn();

    const cube = {
      name: 'legacy_cube',
      cube_id: '',
      is_writable: true,
      relative_path: 'legacy/legacy_cube.cube',
      supported_models: [],
      tags: [],
      mtime: '',
    };
    controller.store.setCubes([cube]);
    controller.store.setFiltered([cube]);
    controller.store.setSelected('legacy/legacy_cube.cube');

    await controller.requestDelete();

    expect(api.delete).not.toHaveBeenCalled();
    expect(controller.toast.push).toHaveBeenCalledWith(
      'error',
      'Delete failed',
      'Cube id missing.',
    );
  });

  test('delete confirmation preserves markup-like cube names as text', async () => {
    const api = createDeleteApi();
    const controller = new CubeBrowserController({ api });
    controller.actions = {
      openConfirmDialog: createConfirmAction(false),
    };
    controller.toast = { push: jest.fn() };
    controller.render = jest.fn();

    const cube = {
      name: '<img src=x onerror=1>',
      cube_id: 'local/example-user/demo.cube',
      is_writable: true,
      relative_path: 'demo.cube',
      supported_models: [],
      tags: [],
      mtime: '',
    };
    controller.store.setCubes([cube]);
    controller.store.setFiltered([cube]);
    controller.store.setSelected(cube.cube_id);

    await controller.requestDelete();

    expect(controller.actions.openConfirmDialog).toHaveBeenCalledWith({
      title: 'Delete SugarCube?',
      message: ['Delete SugarCube "<img src=x onerror=1>"?', 'This cannot be undone.'],
      confirmLabel: 'Delete',
    });
    expect(api.delete).not.toHaveBeenCalled();
  });
});
