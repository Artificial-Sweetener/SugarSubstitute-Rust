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
/** Prove the model catalog keeps one change subscription however consumers come and go. */
import { expect, jest, test } from '@jest/globals';
import { ModelCatalogClient } from '../../../frontend/comfyui/ui/model_library/ModelCatalogClient.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';

/** Return isolated JSON transport values without performing provider traffic. */
function response(payload: unknown): Response {
  return { ok: true, status: 200, json: async () => payload } as Response;
}

/** Count catalog reads and keep every change request pending until it is aborted. */
function longPollTransport() {
  const pending = new Set<AbortSignal>();
  let changeRequests = 0;
  const fetch: ModelApiFetch = async (path, options) => {
    if (path === '/sugarcubes/v1/models')
      return response({ schemaVersion: 1, revision: 4, models: [], fieldContracts: [] });
    changeRequests += 1;
    const signal = options?.signal;
    if (!signal) throw new Error('Change requests must be abortable.');
    pending.add(signal);
    return new Promise<Response>((_resolve, reject) => {
      signal.addEventListener(
        'abort',
        () => {
          pending.delete(signal);
          reject(new DOMException('Cancelled', 'AbortError'));
        },
        { once: true },
      );
    });
  };
  return {
    fetch,
    pending: () => [...pending].filter((signal) => !signal.aborted).length,
    changeRequests: () => changeRequests,
  };
}

/** Let queued catalog reads and the change request start. */
async function settle(): Promise<void> {
  for (let turn = 0; turn < 10; turn++) await Promise.resolve();
}

test('shares one pending change request across consumers and workflow switches', async () => {
  const transport = longPollTransport();
  const client = new ModelCatalogClient(transport.fetch);
  const picker = client.watch(jest.fn(), jest.fn());
  let prompt = client.watch(jest.fn(), jest.fn());
  await settle();
  expect(transport.pending()).toBe(1);

  for (let cycle = 0; cycle < 10; cycle++) {
    prompt();
    prompt = client.watch(jest.fn(), jest.fn());
    await settle();
  }

  expect(transport.changeRequests()).toBe(1);
  expect(transport.pending()).toBe(1);
  prompt();
  picker();
  await settle();
  expect(transport.pending()).toBe(0);
});

test('publishes each committed generation to every consumer from the shared request', async () => {
  let revision = 1;
  let resolveChange: ((value: Response) => void) | null = null;
  const fetch: ModelApiFetch = async (path) => {
    if (path === '/sugarcubes/v1/models')
      return response({ schemaVersion: 1, revision, models: [], fieldContracts: [] });
    return new Promise<Response>((resolve) => {
      resolveChange = resolve;
    });
  };
  const client = new ModelCatalogClient(fetch);
  const first = jest.fn();
  const second = jest.fn();
  const stopFirst = client.watch(first, jest.fn());
  const stopSecond = client.watch(second, jest.fn());
  await settle();
  revision = 2;
  const deliver: unknown = resolveChange;
  if (typeof deliver !== 'function') throw new Error('Missing shared change request.');
  deliver(response({ schemaVersion: 1, revision: 2 }));
  await settle();
  expect(first).toHaveBeenCalledWith(expect.objectContaining({ revision: 2 }));
  expect(second).toHaveBeenCalledWith(expect.objectContaining({ revision: 2 }));
  stopFirst();
  stopSecond();
});
