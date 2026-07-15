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
import { describe, expect, test, beforeEach, jest } from '@jest/globals';
import { CubeBrowserController } from '../../web/comfyui/ui/browser/CubeBrowserController.js';
import { ComfyAdapter } from '../../web/comfyui/ui/core/ComfyAdapter.js';
import type { HostWindow } from '../../web/comfyui/ui/core/ComfyAdapter.js';
import { StorageService } from '../../web/comfyui/ui/core/StorageService.js';

function buildController() {
  const adapter = new ComfyAdapter({
    window: window as HostWindow,
    document,
    storage: localStorage,
    console,
  });
  const storage = new StorageService(adapter);
  const toast = { push: jest.fn() };
  return new CubeBrowserController({ adapter, storage, toast });
}

beforeEach(() => {
  localStorage.clear();
});

describe('cube browser filtering', () => {
  test('search query matches across metadata fields', () => {
    const controller = buildController();
    controller.store.setCubes([
      {
        name: 'Cube Alpha',
        default_alias: 'SDXL/Plain Alpha',
        target_model: 'SDXL',
        description: 'First cube',
        tags: ['one', 'two'],
        supported_models: ['sdxl'],
        author: 'Jane Doe',
        author_url: 'https://example.test',
        cube_id: 'cube-alpha',
        version: '1.2.3',
      },
      {
        name: 'Beta',
        description: 'Second cube',
        tags: ['other'],
        supported_models: ['sd'],
        author: 'Other',
        author_url: '',
        cube_id: 'cube-beta',
        version: '0.9.0',
      },
    ]);
    controller.store.setSearchQuery('jane');
    controller.applyFilters();
    expect(controller.store.state.filtered).toHaveLength(1);
    expect(controller.store.state.filtered[0].name).toBe('Cube Alpha');

    controller.store.setSearchQuery('sdxl/plain');
    controller.applyFilters();
    expect(controller.store.state.filtered).toHaveLength(1);
    expect(controller.store.state.filtered[0].name).toBe('Cube Alpha');

    controller.store.setSearchQuery('cube-beta');
    controller.applyFilters();
    expect(controller.store.state.filtered).toHaveLength(1);
    expect(controller.store.state.filtered[0].name).toBe('Beta');
  });

  test('favorites are sorted ahead of non-favorites', () => {
    const controller = buildController();
    controller.store.setCubes([
      { name: 'Cube A', cube_id: 'cube-a', supported_models: [] },
      { name: 'Cube B', cube_id: 'cube-b', supported_models: [] },
      { name: 'Cube C', cube_id: 'cube-c', supported_models: [] },
    ]);
    controller.store.setFavorites(new Set(['cube-c']));
    controller.store.setSearchQuery('');
    controller.applyFilters();
    expect(controller.store.state.filtered.map((cube) => cube.name)).toEqual([
      'Cube C',
      'Cube A',
      'Cube B',
    ]);
  });
});
