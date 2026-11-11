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
/** Safeguard model-folder configuration and reversible SugarCubes settings actions. */
import {
  decodeModelRootStatus,
  ModelRootClient,
} from '../../../frontend/comfyui/ui/model_library/ModelRootClient.js';
import { ModelLibrarySettings } from '../../../frontend/comfyui/ui/model_library/ModelLibrarySettings.js';
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { FormModalOptions } from '../../../frontend/comfyui/ui/dialogs/FormModal.js';

const status = {
  schemaVersion: 1,
  defaultModelRoot: '/host/models',
  configuredModelRoot: '/shared/models',
  activeModelRoot: '/shared/models',
  usesDefault: false,
  restartRequired: false,
};

/** Model only the external fetch boundary required by the typed client. */
function response(payload: unknown, ok = true): Response {
  return { ok, json: async () => payload } as Response;
}

test('validates root ownership status and preserves useful backend errors', async () => {
  expect(decodeModelRootStatus(status).activeModelRoot).toBe('/shared/models');
  expect(() => decodeModelRootStatus({ ...status, usesDefault: true })).toThrow('invalid');
  expect(() => decodeModelRootStatus({ ...status, schemaVersion: 99 })).toThrow('invalid');
  const requests: RequestInit[] = [];
  const client = new ModelRootClient(async (_path, options) => {
    if (options) requests.push(options);
    return response(status);
  });
  await client.configure('/new/models');
  expect(requests[0]?.method).toBe('PUT');
  expect(requests[0]?.body).toBe(JSON.stringify({ mode: 'custom', path: '/new/models' }));
  const failed = new ModelRootClient(async () =>
    response({ error: 'Model root must be absolute.' }, false),
  );
  await expect(failed.load()).rejects.toThrow('Model root must be absolute.');
});

test('canceling the folder dialog never persists a selection', async () => {
  let render: (() => unknown) | null = null;
  const requests: string[] = [];
  let form: FormModalOptions | null = null;
  let resolveOpened: (() => void) | null = null;
  const opened = new Promise<void>((resolve) => {
    resolveOpened = resolve;
  });
  const rootClient = new ModelRootClient(async (_path, options) => {
    requests.push(options?.method ?? 'GET');
    return response(status);
  });
  const settings = new ModelLibrarySettings({
    app: {
      ui: {
        settings: {
          addSetting(value: unknown): void {
            if (isRecord(value) && typeof value.type === 'function') {
              const type = value.type;
              render = () => type();
            }
          },
        },
      },
    },
    document,
    roots: rootClient,
    catalog: new ModelCatalogClient(async () => response({})),
    dialogs: {
      openForm: async (options) => {
        form = options ?? null;
        resolveOpened?.();
        return null;
      },
      alert: async () => true,
    },
    reportError: (error) => {
      throw error;
    },
  });
  settings.setup();
  settings.setup();
  // The host invokes a dynamic settings renderer; narrow its result at that boundary.
  const renderer: unknown = render;
  if (typeof renderer !== 'function') throw new Error('Settings row was not registered.');
  const row: unknown = renderer();
  if (!(row instanceof HTMLElement)) throw new Error('Settings row is invalid.');
  document.body.append(row);
  const button = row.querySelector('button');
  if (!button) throw new Error('The model folder action is missing.');
  button.click();
  await opened;
  expect(requests).toEqual(['GET']);
  expect(form).toEqual(
    expect.objectContaining({ title: 'Model folder', confirmLabel: 'Save model folder' }),
  );
  row.remove();
});
