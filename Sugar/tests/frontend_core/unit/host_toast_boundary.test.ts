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
/** Prove notifications use validated public Comfy host services with their original receiver. */
import { afterEach, describe, expect, jest, test } from '@jest/globals';
import { ComfyAdapter } from '../../../frontend/comfyui/ui/core/ComfyAdapter.js';
import type { HostToast, HostWindow } from '../../../frontend/comfyui/ui/core/ComfyAdapter.js';
import { ToastService } from '../../../frontend/comfyui/ui/core/ToastService.js';
import type { ComfyApplication } from '../../../frontend/comfyui/ui/types/graph.js';

type ToastMessage = Parameters<HostToast['add']>[0];

const hostWindow = window as HostWindow;

afterEach(() => {
  delete hostWindow.comfyAPI;
  delete hostWindow.app;
});

describe('Comfy host toast boundary', () => {
  test('delivers once through the public extension manager and preserves its receiver', () => {
    const calls: ToastMessage[] = [];
    const modernToast = {
      /** Retain the host store receiver when accepting a notification. */
      add(message: ToastMessage): void {
        expect(this).toBe(modernToast);
        calls.push(message);
      },
    };
    const legacyAdd = jest.fn<HostToast['add']>();
    hostWindow.comfyAPI = {
      vueApp: { config: { globalProperties: { $toast: { add: legacyAdd } } } },
    };
    const app: ComfyApplication = { extensionManager: { toast: modernToast } };
    const toast = new ToastService(new ComfyAdapter({ app, window: hostWindow }));

    toast.push('info', 'Switched to graph view', 'Masonry requires Nodes 2.0.');

    expect(calls).toEqual([
      {
        severity: 'info',
        summary: 'Switched to graph view',
        detail: 'Masonry requires Nodes 2.0.',
        life: 5000,
      },
    ]);
    expect(legacyAdd).not.toHaveBeenCalled();
  });

  test('resolves the public notification store after adapter construction', () => {
    const app: ComfyApplication = {};
    const adapter = new ComfyAdapter({ app, window: hostWindow });
    expect(adapter.getToast()).toBeNull();
    const add = jest.fn<HostToast['add']>();
    app.extensionManager = { toast: { add } };

    new ToastService(adapter).push('success', 'Saved', 'Workflow saved.');

    expect(add).toHaveBeenCalledTimes(1);
  });

  test('preserves the older host API when a public manager is unavailable', () => {
    const calls: ToastMessage[] = [];
    const legacyToast = {
      /** Keep older PrimeVue notification services bound to their owner. */
      add(message: ToastMessage): void {
        expect(this).toBe(legacyToast);
        calls.push(message);
      },
    };
    hostWindow.comfyAPI = {
      vueApp: { config: { globalProperties: { $toast: legacyToast } } },
    };

    new ToastService(new ComfyAdapter({ window: hostWindow })).push('info', 'Ready', '');

    expect(calls).toHaveLength(1);
    expect(calls[0]?.summary).toBe('Ready');
  });

  test.each([null, 'invalid', { toast: null }, { toast: { add: 'invalid' } }])(
    'rejects malformed public services without opening a dialog: %p',
    (extensionManager) => {
      const browserAlert = jest.fn<typeof alert>();
      const dialogAlert = jest.fn(() => Promise.resolve(true));
      const info = jest.fn<Console['info']>();
      const adapter = new ComfyAdapter({
        app: { extensionManager },
        window: hostWindow,
        alert: browserAlert,
        console: { ...console, info },
      });

      new ToastService(adapter, { dialogs: { alert: dialogAlert } }).push('info', 'Ready', '');

      expect(adapter.getToast()).toBeNull();
      expect(info).toHaveBeenCalledWith('Ready');
      expect(browserAlert).not.toHaveBeenCalled();
      expect(dialogAlert).not.toHaveBeenCalled();
    },
  );
});
