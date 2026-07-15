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
import { describe, expect, jest, test } from '@jest/globals';
import { CubeLibraryApi } from '../../web/comfyui/ui/core/CubeLibraryApi.js';
import type { ApiJsonResult } from '../../web/comfyui/ui/core/CubeLibraryApi.js';
import type { UnknownRecord } from '../../web/comfyui/ui/types/common.js';

interface CapturedRequest {
  url: string;
  options?: RequestInit;
}

type EndpointCase = readonly [
  name: string,
  invoke: (api: CubeLibraryApi) => Promise<ApiJsonResult>,
  url: string,
  options: RequestInit,
];

function createApi(data: UnknownRecord = { ok: true }) {
  const requests: CapturedRequest[] = [];
  const fetchApi = jest.fn(async (url: string, options?: RequestInit) => {
    requests.push({ url, options });
    return {
      ok: true,
      status: 200,
      json: async () => data,
    };
  });
  return {
    api: new CubeLibraryApi({ getApi: () => ({ fetchApi }) }),
    fetchApi,
    requests,
  };
}

const payload = '{"cube_id":"local/example-user/demo.cube"}';
const requestOptions = { headers: { 'Content-Type': 'application/json' } };

const endpointCases: EndpointCase[] = [
  ['list', (api) => api.list({ cache: 'reload' }), '/sugarcubes/list', { cache: 'reload' }],
  ['tracked repos', (api) => api.listTrackedRepos(), '/sugarcubes/repos', {}],
  ['identity policy', (api) => api.getIdentityPolicy(), '/sugarcubes/identity_policy', {}],
  [
    'identity policy update',
    (api) => api.updateIdentityPolicy(payload, requestOptions),
    '/sugarcubes/identity_policy',
    { method: 'PATCH', body: payload, ...requestOptions },
  ],
  [
    'tracked repo add',
    (api) => api.addTrackedRepo(payload, requestOptions),
    '/sugarcubes/repos',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'tracked repo preflight',
    (api) => api.preflightCubePack(payload, requestOptions),
    '/sugarcubes/repos/preflight',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'authoring repo creation',
    (api) => api.createAuthoringCubePack(payload, requestOptions),
    '/sugarcubes/repos/authoring',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'tracked repo update',
    (api) => api.updateTrackedRepo(payload, requestOptions),
    '/sugarcubes/repos',
    { method: 'PATCH', body: payload, ...requestOptions },
  ],
  [
    'tracked repo sync',
    (api) => api.syncTrackedRepo(payload, requestOptions),
    '/sugarcubes/repos/sync',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'all tracked repo sync',
    (api) => api.syncAllTrackedRepos(requestOptions),
    '/sugarcubes/repos/sync_all',
    { method: 'POST', ...requestOptions },
  ],
  [
    'pack check',
    (api) => api.checkCubePack(payload, requestOptions),
    '/sugarcubes/packs/check',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'all pack check',
    (api) => api.checkAllCubePacks(payload, requestOptions),
    '/sugarcubes/packs/check_all',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'preview',
    (api) => api.preview('local/example user/demo.cube'),
    '/sugarcubes/preview?cube_id=local%2Fexample%20user%2Fdemo.cube',
    {},
  ],
  [
    'revisions',
    (api) => api.listRevisions('local/example user/demo.cube'),
    '/sugarcubes/revisions?cube_id=local%2Fexample%20user%2Fdemo.cube',
    {},
  ],
  [
    'load',
    (api) => api.load(payload, requestOptions),
    '/sugarcubes/load',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'revision load',
    (api) => api.loadRevision(payload, requestOptions),
    '/sugarcubes/load_revision',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'rename',
    (api) => api.rename(payload, requestOptions),
    '/sugarcubes/rename',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'promote',
    (api) => api.promote(payload, requestOptions),
    '/sugarcubes/promote',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'metadata update',
    (api) => api.updateMetadata(payload, requestOptions),
    '/sugarcubes/update_metadata',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'bulk save',
    (api) => api.saveMany(payload, requestOptions),
    '/sugarcubes/save_many',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'implementation save',
    (api) => api.saveImplementation(payload, requestOptions),
    '/sugarcubes/save_implementation',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'authored flavor save',
    (api) => api.saveAuthoredFlavor(payload, requestOptions),
    '/sugarcubes/save_authored_flavor',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'local flavor list',
    (api) => api.getLocalFlavors('local/example user/demo.cube', requestOptions),
    '/sugarcubes/local_flavors?cube_id=local%2Fexample%20user%2Fdemo.cube',
    requestOptions,
  ],
  [
    'local flavor save',
    (api) => api.saveLocalFlavor(payload, requestOptions),
    '/sugarcubes/local_flavors',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'local flavor delete',
    (api) => api.deleteLocalFlavor(payload, requestOptions),
    '/sugarcubes/local_flavors',
    { method: 'DELETE', body: payload, ...requestOptions },
  ],
  [
    'local flavor selection',
    (api) => api.selectLocalFlavor(payload, requestOptions),
    '/sugarcubes/local_flavors/select',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'local flavor migration',
    (api) => api.migrateLocalFlavors(payload, requestOptions),
    '/sugarcubes/local_flavors/migrate',
    { method: 'POST', body: payload, ...requestOptions },
  ],
  [
    'local flavor reconciliation',
    (api) => api.reconcileLocalFlavors(payload, requestOptions),
    '/sugarcubes/local_flavors/reconcile',
    { method: 'POST', body: payload, ...requestOptions },
  ],
];

describe('cube library API contract', () => {
  test.each(endpointCases)(
    '%s preserves its route and request shape',
    async (_name, invoke, url, options) => {
      const { api, requests } = createApi();

      const result = await invoke(api);

      expect(result.data).toEqual({ ok: true });
      expect(requests).toEqual([{ url, options }]);
    },
  );

  test('repo removal encodes identity while preserving an empty-reference fallback', async () => {
    const { api, requests } = createApi();

    await api.removeTrackedRepo({ owner: 'Artificial Sweetener', repo: 'Base/Cubes' });
    await api.removeTrackedRepo();

    expect(requests).toEqual([
      {
        url: '/sugarcubes/repos?owner=Artificial+Sweetener&repo=Base%2FCubes',
        options: { method: 'DELETE' },
      },
      { url: '/sugarcubes/repos', options: { method: 'DELETE' } },
    ]);
  });

  test('cube deletion supports string, object, and empty references', async () => {
    const { api, requests } = createApi();

    await api.delete('local/example user/demo.cube');
    await api.delete({ cube_id: 'local/example-user/second.cube' });
    await api.delete(null);

    expect(requests).toEqual([
      {
        url: '/sugarcubes?cube_id=local%2Fexample%20user%2Fdemo.cube',
        options: { method: 'DELETE' },
      },
      {
        url: '/sugarcubes?cube_id=local%2Fexample-user%2Fsecond.cube',
        options: { method: 'DELETE' },
      },
      { url: '/sugarcubes', options: { method: 'DELETE' } },
    ]);
  });

  test('public pack aliases preserve their tracked-repository contracts', async () => {
    const { api, requests } = createApi();

    await api.listCubePacks();
    await api.addCubePack(payload);
    await api.updateCubePack(payload);
    await api.removeCubePack({ owner: 'owner', repo: 'repo' });
    await api.updateCubePackNow(payload);
    await api.updateAllCubePacks();

    expect(requests.map(({ url, options }) => [url, options?.method || 'GET'])).toEqual([
      ['/sugarcubes/repos', 'GET'],
      ['/sugarcubes/repos', 'POST'],
      ['/sugarcubes/repos', 'PATCH'],
      ['/sugarcubes/repos?owner=owner&repo=repo', 'DELETE'],
      ['/sugarcubes/repos/sync', 'POST'],
      ['/sugarcubes/repos/sync_all', 'POST'],
    ]);
  });

  test('caller options retain their established ability to override request defaults', async () => {
    const { api, requests } = createApi();

    await api.saveMany(payload, { method: 'PUT', body: 'replacement' });

    expect(requests[0]).toEqual({
      url: '/sugarcubes/save_many',
      options: { method: 'PUT', body: 'replacement' },
    });
  });

  test('failed JSON parsing produces an empty data object while preserving the response', async () => {
    const response = {
      ok: false,
      status: 502,
      json: jest.fn(() => Promise.reject(new SyntaxError('invalid json'))),
    };
    const api = new CubeLibraryApi({
      getApi: () => ({ fetchApi: jest.fn(async () => response) }),
    });

    await expect(api.list()).resolves.toEqual({ response, data: {} });
  });

  test('missing ComfyUI API fails before attempting a request', async () => {
    const api = new CubeLibraryApi({ getApi: () => null });

    await expect(api.list()).rejects.toThrow('ComfyUI API unavailable');
  });
});
