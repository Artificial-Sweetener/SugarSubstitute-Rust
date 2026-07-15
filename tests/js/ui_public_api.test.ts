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
import { app } from './mocks/app.js';
import { api } from './mocks/api.js';

async function loadUi() {
  const cacheBust = `?v=${Math.random().toString(36).slice(2)}`;
  return import(`../../web/comfyui/ui.js${cacheBust}`);
}

beforeEach(() => {
  app.reset();
  api.fetchApi = async () => ({ ok: true, json: async () => ({ ok: true }) });
  globalThis.requestAnimationFrame = (callback) => {
    callback(0);
    return 1;
  };
  globalThis.cancelAnimationFrame = () => {};
  globalThis.window = {
    SugarCubes: {} as SugarCubesPublicApi,
    comfyAPI: { vueApp: { config: { globalProperties: { $toast: null } } } },
  } as unknown as Window & typeof globalThis;
  globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;
  globalThis.window.cancelAnimationFrame = globalThis.cancelAnimationFrame;
});

describe('ui public api', () => {
  test('registers extension and exposes SugarCubes api', async () => {
    await loadUi();
    expect(app._extensions).toHaveLength(1);
    expect(typeof window.SugarCubes.listCubes).toBe('function');
    expect(typeof window.SugarCubes.previewCube).toBe('function');
    expect(typeof window.SugarCubes.openLibrary).toBe('function');
    expect(typeof window.SugarCubes.scheduleCubeInstanceRefresh).toBe('function');
    expect(typeof window.SugarCubes.scheduleCubeDirtyRefresh).toBe('function');
    const keys = Object.keys(window.SugarCubes).sort();
    expect(keys).toEqual([
      'listCubes',
      'openLibrary',
      'previewCube',
      'scheduleCubeDirtyRefresh',
      'scheduleCubeInstanceRefresh',
    ]);
    expect(Object.isFrozen(window.SugarCubes)).toBe(true);
  });

  test('listCubes and previewCube use expected urls', async () => {
    const logSpy = jest.spyOn(console, 'log').mockImplementation(() => {});
    const calls: string[] = [];
    api.fetchApi = async (url) => {
      calls.push(url);
      return { ok: true, json: async () => ({ ok: true }) };
    };

    await loadUi();
    const list = await window.SugarCubes.listCubes();
    const preview = await window.SugarCubes.previewCube('local/example-user/demo.cube');

    expect(list.ok).toBe(true);
    expect(preview.ok).toBe(true);
    expect(calls[0]).toBe('/sugarcubes/list');
    expect(calls[1]).toBe('/sugarcubes/preview?cube_id=local%2Fexample-user%2Fdemo.cube');
    logSpy.mockRestore();
  });

  test('schedule refresh calls requestAnimationFrame', async () => {
    const rafCalls: FrameRequestCallback[] = [];
    globalThis.requestAnimationFrame = (callback) => {
      rafCalls.push(callback);
      callback(0);
      return 1;
    };
    globalThis.window.requestAnimationFrame = globalThis.requestAnimationFrame;

    await loadUi();
    window.SugarCubes.scheduleCubeInstanceRefresh();
    window.SugarCubes.scheduleCubeDirtyRefresh();
    expect(typeof window.SugarCubesDebug?.getDirtyState).toBe('function');

    expect(rafCalls.length).toBe(2);
  });
});
