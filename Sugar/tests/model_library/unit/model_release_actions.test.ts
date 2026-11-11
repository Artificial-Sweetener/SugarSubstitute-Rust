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
/** Prove release decoding, explicit acquisition, and host-choice refresh preserve workflow values. */
import { jest } from '@jest/globals';
import { modelApiError } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import { ModelJobClient } from '../../../frontend/comfyui/ui/model_library/ModelJobClient.js';
import { decodeModelReleases } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';
import { refreshComfyModelChoices } from '../../../frontend/comfyui/ui/model_library/ComfyModelChoices.js';
import type { ModelTransferObserver } from '../../../frontend/comfyui/ui/model_library/ModelTransfer.js';

/** Supply an isolated HTTP response at the external transport boundary. */
function response(payload: unknown): Response {
  return { ok: true, status: 200, json: async () => payload } as Response;
}

test('keeps release labels literal and rejects malformed artifact evidence', () => {
  const versions = [
    {
      versionId: '20',
      name: '<b>v2</b>',
      baseModel: 'SDXL',
      publishedAt: '',
      pageUrl: 'https://civitai.com/models/10',
      files: [
        {
          access: 'public',
          fileId: '30',
          fileName: 'fixture.safetensors',
          sha256: 'a'.repeat(64),
          sizeBytes: 10,
          kind: 'loras',
        },
      ],
    },
  ];
  expect(decodeModelReleases({ schemaVersion: 1, versions })[0]?.name).toBe('<b>v2</b>');
  expect(decodeModelReleases({ schemaVersion: 1, versions })[0]?.inUse).toBeNull();
  expect(
    decodeModelReleases({ schemaVersion: 1, versions: [{ ...versions[0], inUse: true }] })[0]
      ?.inUse,
  ).toBe(true);
  expect(() =>
    decodeModelReleases({ schemaVersion: 1, versions: [{ ...versions[0], inUse: 'true' }] }),
  ).toThrow('current model release');
  expect(() => decodeModelReleases({ schemaVersion: 2, versions })).toThrow();
  expect(() =>
    decodeModelReleases({
      schemaVersion: 1,
      versions: [{ ...versions[0], versionId: '../private' }],
    }),
  ).toThrow();
  expect(() =>
    decodeModelReleases({
      schemaVersion: 1,
      versions: [{ ...versions[0], files: [{ fileId: '30', sha256: 'bad' }] }],
    }),
  ).toThrow();
});

test('acquires by offered IDs and refreshes native choices only after verified completion', async () => {
  const requests: { path: string; body: string | null }[] = [];
  const installed = jest.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const client = new ModelCatalogClient(async (path, options) => {
    requests.push({ path, body: typeof options?.body === 'string' ? options.body : null });
    if (path.endsWith('/acquire'))
      return response({
        jobId: 'a'.repeat(32),
        status: 'completed',
        result: { status: 'installed', fileName: 'fixture.safetensors' },
      });
    return response({ schemaVersion: 1, revision: 1, models: [], fieldContracts: [] });
  }, installed);
  await expect(
    client.acquire('b'.repeat(64), '20', '30', () => undefined, new AbortController().signal),
  ).resolves.toBe('fixture.safetensors');
  expect(requests[0]?.body).toBe(JSON.stringify({ versionId: '20', fileId: '30' }));
  expect(installed).toHaveBeenCalledTimes(1);
  const failed = new ModelCatalogClient(
    async () => response({ jobId: 'a'.repeat(32), status: 'completed', result: {} }),
    installed,
  );
  await expect(
    failed.acquire('b'.repeat(64), '20', '30', () => undefined, new AbortController().signal),
  ).rejects.toThrow('Invalid model download');
  expect(installed).toHaveBeenCalledTimes(1);
});

test('cancels a server job even when its response arrives after the view has closed', async () => {
  const requests: string[] = [];
  const jobs = new ModelJobClient(async (path) => {
    requests.push(path);
    return response({});
  });
  const controller = new AbortController();
  controller.abort();
  await expect(
    jobs.observe(
      response({ jobId: 'a'.repeat(32), status: 'running' }),
      { queued: 'queued', running: 'running' },
      () => undefined,
      controller.signal,
    ),
  ).rejects.toThrow('Cancelled');
  expect(requests).toEqual([`/sugarcubes/v1/models/jobs/${'a'.repeat(32)}`]);
});

test('reports shared transfer progress and rejects malformed byte evidence', async () => {
  jest.useFakeTimers();
  try {
    const progress = jest.fn<(message: string) => void>();
    const transfer = jest.fn<ModelTransferObserver>();
    const jobs = new ModelJobClient(async () =>
      response({ jobId: 'a'.repeat(32), status: 'completed', result: { status: 'installed' } }),
    );
    const observed = jobs.observe(
      response({
        jobId: 'a'.repeat(32),
        status: 'running',
        progress: { bytesDownloaded: 25, bytesTotal: 100 },
      }),
      { queued: 'Queued', running: 'Downloading' },
      progress,
      new AbortController().signal,
      transfer,
    );
    await jest.advanceTimersByTimeAsync(400);
    await expect(observed).resolves.toEqual({ status: 'installed' });
    expect(progress).toHaveBeenCalledWith('Downloading 25%');
    expect(transfer).toHaveBeenCalledWith({ message: 'Downloading', percentage: 25 });
    await expect(
      jobs.observe(
        response({
          jobId: 'a'.repeat(32),
          status: 'running',
          progress: { bytesDownloaded: -1, bytesTotal: 100 },
        }),
        { queued: 'Queued', running: 'Downloading' },
        progress,
        new AbortController().signal,
      ),
    ).rejects.toThrow('Invalid model transfer progress');
  } finally {
    jest.useRealTimers();
  }
});

test('calls the host refresh lifecycle without changing native selected values', async () => {
  const app = {
    selected: 'installed.safetensors',
    refreshComboInNodes: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
  };
  await refreshComfyModelChoices(app);
  expect(app.refreshComboInNodes).toHaveBeenCalledTimes(1);
  expect(app.selected).toBe('installed.safetensors');
  await expect(refreshComfyModelChoices({})).rejects.toThrow('model was downloaded');
});

test('keeps provider connection failures accurate and response labels literal', async () => {
  const message = 'The model provider connection failed. Check network or certificate settings.';
  const client = new ModelCatalogClient(
    async () => ({ ok: false, json: async () => ({ error: message }) }) as Response,
  );
  await expect(client.releases('a'.repeat(64), new AbortController().signal)).rejects.toThrow(
    message,
  );
  expect((await modelApiError(response({ error: '<b>literal</b>' }), 'fallback')).message).toBe(
    '<b>literal</b>',
  );
  expect((await modelApiError(response({ error: 'x'.repeat(501) }), 'fallback')).message).toBe(
    'fallback',
  );
});
