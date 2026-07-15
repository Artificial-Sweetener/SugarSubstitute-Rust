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
import { ComfyAdapter } from '../../web/comfyui/ui/core/ComfyAdapter.js';
import type { HostToast, HostWindow } from '../../web/comfyui/ui/core/ComfyAdapter.js';
import type { ComfyApplication, ComfyHostApi } from '../../web/comfyui/ui/types/graph.js';
import { CubeLibraryApi } from '../../web/comfyui/ui/core/CubeLibraryApi.js';
import { EventBus } from '../../web/comfyui/ui/core/EventBus.js';
import { StorageService } from '../../web/comfyui/ui/core/StorageService.js';
import { ToastService } from '../../web/comfyui/ui/core/ToastService.js';

describe('core services', () => {
  beforeEach(() => {
    localStorage.clear();
    delete window.app;
    delete window.api;
  });

  test('EventBus emits payloads', () => {
    const bus = new EventBus();
    let received: unknown = null;
    bus.on('demo', (value) => {
      received = value;
    });
    bus.emit('demo', { ok: true });
    expect(received).toEqual({ ok: true });
  });

  test('StorageService reads and writes lists and sets', () => {
    const storage = new StorageService({ getStorage: () => localStorage });
    storage.writeList('list', ['a', null, 'b']);
    expect(storage.readList('list')).toEqual(['a', 'b']);

    storage.writeSet('set', new Set(['x', '', 'y']));
    expect(Array.from(storage.readSet('set'))).toEqual(['x', 'y']);
  });

  test('ToastService forwards to toast adapter', () => {
    const calls: Parameters<HostToast['add']>[0][] = [];
    const adapter = {
      getToast: () => ({ add: (payload: Parameters<HostToast['add']>[0]) => calls.push(payload) }),
    };
    const toast = new ToastService(adapter);
    toast.push('success', 'Saved', 'done');
    expect(calls).toHaveLength(1);
    expect(calls[0].summary).toBe('Saved');
  });

  test('ToastService routes fallback errors through dialogs instead of browser alert', () => {
    const consoleRef = { error: jest.fn() };
    const browserAlert = jest.fn();
    const dialogAlert = jest.fn(() => Promise.resolve(true));
    const toast = new ToastService(
      {
        getToast: () => null,
        getConsole: () => consoleRef,
        getAlert: () => browserAlert,
      },
      { dialogs: { alert: dialogAlert } },
    );

    toast.push('error', 'Create failed', 'Unable to serialize the current graph.');

    expect(consoleRef.error).toHaveBeenCalledWith(
      'Create failed: Unable to serialize the current graph.',
    );
    expect(dialogAlert).toHaveBeenCalledWith({
      title: 'Create failed',
      message: ['Unable to serialize the current graph.'],
      confirmLabel: 'OK',
    });
    expect(browserAlert).not.toHaveBeenCalled();
  });

  test('ComfyAdapter resolves app and api after construction', () => {
    const hostWindow = window as HostWindow;
    const adapter = new ComfyAdapter({ window: hostWindow });
    const graph = {};
    const canvas = {};
    const api: ComfyHostApi = {};

    hostWindow.app = { graph, canvas } as ComfyApplication;
    hostWindow.api = api;

    expect(adapter.getApp()).toBe(hostWindow.app);
    expect(adapter.getGraph()).toBe(graph);
    expect(adapter.getCanvas()).toBe(canvas);
    expect(adapter.getApi()).toBe(api);
  });

  test('CubeLibraryApi wraps fetch responses', async () => {
    const api = new CubeLibraryApi({
      getApi: () => ({
        fetchApi: async () => ({
          json: async () => ({ ok: true }),
        }),
      }),
    });
    const result = await api.list();
    expect(result.data).toEqual({ ok: true });
  });

  test('CubeLibraryApi exposes revision routes', async () => {
    const requests: Array<{ url: string; options: RequestInit | undefined }> = [];
    const api = new CubeLibraryApi({
      getApi: () => ({
        fetchApi: async (url: string, options?: RequestInit) => {
          requests.push({ url, options });
          return { json: async () => ({ ok: true }) };
        },
      }),
    });

    await api.listRevisions('Artificial-Sweetener/Base-Cubes/text to image.cube');
    await api.loadRevision(JSON.stringify({ cube_id: 'cube-a', revision_ref: 'abc123' }), {
      headers: { 'Content-Type': 'application/json' },
    });

    expect(requests).toHaveLength(2);
    expect(requests[0].url).toBe(
      '/sugarcubes/revisions?cube_id=Artificial-Sweetener%2FBase-Cubes%2Ftext%20to%20image.cube',
    );
    expect(requests[1].url).toBe('/sugarcubes/load_revision');
    expect(requests[1]!.options?.method).toBe('POST');
  });
});
