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
/** Prove explicit metadata and thumbnail actions use the authoritative backend contract. */
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import { jest } from '@jest/globals';
import { decodeModelCatalog } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';

/** Build only validated catalog identities for client priority contracts. */
function priorityChoices(ids: readonly string[]) {
  return decodeModelCatalog({
    schemaVersion: 1,
    revision: 0,
    fieldContracts: [],
    models: ids.map((id) => ({
      id,
      kind: 'loras',
      value: id,
      rootId: '',
      relativePath: '',
      folder: '',
      title: '',
      version: '',
      baseModel: '',
      provider: '',
      providerUrl: '',
      description: '',
      trainedWords: [],
      thumbnails: [],
    })),
  }).items;
}

/** Return isolated JSON transport values without performing provider traffic. */
function response(payload: unknown, status = 200): Response {
  return { ok: status === 200 || status === 202, status, json: async () => payload } as Response;
}

test('observes completed metadata without changing any workflow field', async () => {
  const requests: string[] = [];
  const fetch: ModelApiFetch = async (path) => {
    requests.push(path);
    if (path.endsWith('/metadata/refresh'))
      return response(
        { jobId: 'a'.repeat(32), status: 'completed', result: { status: 'not_found' } },
        202,
      );
    return response({ schemaVersion: 1, revision: 1, models: [], fieldContracts: [] });
  };
  const client = new ModelCatalogClient(fetch);
  const progress: string[] = [];
  await client.refreshMetadata(
    'b'.repeat(64),
    (message) => progress.push(message),
    new AbortController().signal,
  );
  expect(progress).toEqual(['No provider match was found.']);
  expect(requests).toEqual([
    `/sugarcubes/v1/models/${'b'.repeat(64)}/metadata/refresh`,
    '/sugarcubes/v1/models',
  ]);
});

test('prioritizes bounded unique catalog identities and retries failed requests', async () => {
  const requests: { path: string; options: RequestInit | undefined }[] = [];
  let failed = true;
  const client = new ModelCatalogClient(async (path, options) => {
    requests.push({ path, options });
    return response({}, failed ? 503 : 202);
  });
  const identities = Array.from({ length: 130 }, (_, index) =>
    index.toString(16).padStart(64, '0'),
  );
  const choices = priorityChoices(['literal:None', '0'.repeat(64), ...identities]);
  await expect(client.prioritize(choices)).rejects.toThrow('could not be prioritized');
  failed = false;
  await client.prioritize(choices);
  expect(requests).toHaveLength(2);
  expect(requests[1]?.path).toBe('/sugarcubes/v1/models/metadata/prioritize');
  expect(requests[1]?.options?.body).toBe(JSON.stringify({ modelIds: identities.slice(0, 128) }));
  await client.prioritize(choices);
  expect(requests[2]?.options?.body).toBe(JSON.stringify({ modelIds: identities.slice(128) }));
  await client.prioritize(choices);
  expect(requests).toHaveLength(3);
});

test('rejects corrupt job identities and reports provider failure', async () => {
  const malformed = new ModelCatalogClient(async () =>
    response({ jobId: '../secret', status: 'completed' }),
  );
  await expect(
    malformed.refreshMetadata('b'.repeat(64), () => undefined, new AbortController().signal),
  ).rejects.toThrow('Invalid model job');
  const failed = new ModelCatalogClient(async () =>
    response({ jobId: 'a'.repeat(32), status: 'failed', error: 'Provider cooldown.' }),
  );
  await expect(
    failed.refreshMetadata('b'.repeat(64), () => undefined, new AbortController().signal),
  ).rejects.toThrow('Provider cooldown.');
});

test('reconnects catalog updates after transient failures and cancels retry timers', async () => {
  jest.useFakeTimers();
  try {
    let attempts = 0;
    let reads = 0;
    const fetch: ModelApiFetch = async (path, options) => {
      if (path === '/sugarcubes/v1/models')
        return response({ schemaVersion: 1, revision: reads++, models: [], fieldContracts: [] });
      attempts += 1;
      if (attempts <= 2) return response({}, 503);
      if (attempts === 3) return response({ schemaVersion: 1, revision: 1 });
      return new Promise<Response>((_, reject) => {
        options?.signal?.addEventListener(
          'abort',
          () => reject(new DOMException('Cancelled', 'AbortError')),
          { once: true },
        );
      });
    };
    const changed = jest.fn();
    const errors = jest.fn();
    const stop = new ModelCatalogClient(fetch).watch(changed, errors);
    await jest.advanceTimersByTimeAsync(0);
    expect(attempts).toBe(1);
    await jest.advanceTimersByTimeAsync(1000);
    expect(attempts).toBe(2);
    await jest.advanceTimersByTimeAsync(2000);
    expect(changed).toHaveBeenCalledWith(expect.objectContaining({ revision: 1 }));
    expect(errors).toHaveBeenCalledTimes(1);
    stop();
    await jest.advanceTimersByTimeAsync(30_000);
    expect(attempts).toBe(4);

    const retry = new ModelCatalogClient(async () => {
      throw new Error('Disconnected');
    });
    const cancel = retry.watch(jest.fn(), jest.fn());
    await jest.advanceTimersByTimeAsync(0);
    cancel();
    await jest.advanceTimersByTimeAsync(0);
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});
