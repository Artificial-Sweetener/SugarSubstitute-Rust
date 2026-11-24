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
import { drawRows, settle } from '../../settings/support/SettingsRowHarness.js';

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

test('an abandoned model folder edit never persists; Enter saves it once', async () => {
  const requests: { method: string; body?: string }[] = [];
  const rootClient = new ModelRootClient(async (_path, options) => {
    requests.push({
      method: options?.method ?? 'GET',
      ...(typeof options?.body === 'string' ? { body: options.body } : {}),
    });
    return response(status);
  });
  const errors: unknown[] = [];
  const app = {};
  const settings = new ModelLibrarySettings({
    app,
    roots: rootClient,
    catalog: new ModelCatalogClient(async () =>
      response({ schemaVersion: 1, pattern: '{architecture}/{file_name}' }),
    ),
    reportError: (error) => errors.push(error),
  });
  settings.setup();
  settings.setup();
  await settle();
  const rows = drawRows(app, ['comfyui.connection.model_folder'], (error) => errors.push(error));
  const field = rows.control<HTMLInputElement>('comfyui.connection.model_folder', 'input');
  expect(field.value).toBe('/shared/models');
  field.focus();
  field.value = '/elsewhere/models';
  field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await settle();
  expect(field.value).toBe('/shared/models');
  expect(requests.map((request) => request.method)).toEqual(['GET']);
  field.focus();
  field.value = '/new/models';
  field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  field.dispatchEvent(new Event('change'));
  await settle();
  await settle();
  expect(requests.filter((request) => request.method === 'PUT')).toEqual([
    { method: 'PUT', body: JSON.stringify({ mode: 'custom', path: '/new/models' }) },
  ]);
  expect(errors).toEqual([]);
  rows.dispose();
});
