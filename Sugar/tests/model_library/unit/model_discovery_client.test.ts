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
/** Safeguard reviewed discovery boundaries, shared jobs, and secure credential commands. */
import { jest } from '@jest/globals';
import { ModelDiscoveryClient } from '../../../frontend/comfyui/ui/model_library/ModelDiscoveryClient.js';
import { CivitaiCredentialClient } from '../../../frontend/comfyui/ui/model_library/CivitaiCredentialClient.js';
import {
  decodeDiscoveryContext,
  decodeDiscoveryPlan,
} from '../../../frontend/comfyui/ui/model_library/ModelDiscoveryCodec.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import { discoveryPlan } from '../support/model_discovery.js';

/** Represent only the fetch boundary consumed by these clients. */
function response(payload: unknown, ok = true): Response {
  return { ok, json: async () => payload } as Response;
}

/** Wrap reviewed application results in the shared completed-job contract. */
function job(result: unknown, status = 'completed'): Response {
  return response({ jobId: 'c'.repeat(32), status, result, error: '', progress: {} });
}

test('validates bounded plans, literal metadata, ordered alternate sources, and server-only previews', () => {
  const plan = discoveryPlan();
  expect(decodeDiscoveryContext({ schemaVersion: 1, context: null })).toBeNull();
  expect(
    decodeDiscoveryContext({
      schemaVersion: 1,
      context: { artifactKind: 'upscale_models', familyId: null },
    }),
  ).toEqual({ artifactKind: 'upscale_models', familyId: null });
  expect(decodeDiscoveryPlan({ schemaVersion: 1, ...plan })).toEqual(plan);
  const item = plan.suggestions[0],
    offer = item?.offers[0];
  if (!item || !offer) throw new Error('Missing reviewed test offer.');
  const decode = (change: Record<string, unknown>): void => {
    decodeDiscoveryPlan({
      schemaVersion: 1,
      ...plan,
      suggestions: [{ ...item, offers: [{ ...offer, ...change }] }],
    });
  };
  expect(() =>
    decode({ previewUrl: `/sugarcubes/v1/models/discovery/${plan.planId}/preview/0/openmodeldb` }),
  ).not.toThrow();
  for (const value of [
    'https://foreign.test/preview',
    `/sugarcubes/v1/models/discovery/${'d'.repeat(32)}/preview/0/openmodeldb`,
    `/sugarcubes/v1/models/discovery/${plan.planId}/preview/0/civitai`,
    `/sugarcubes/v1/models/discovery/${plan.planId}/preview/../openmodeldb`,
  ])
    expect(() => decode({ previewUrl: value })).toThrow('Invalid model preview');
  for (const value of [
    'javascript:alert(1)',
    'https://civitai.com.evil.test/models/1',
    'http://civitai.com/models/1',
    'https://key@civitai.com/models/1',
  ])
    expect(() => decode({ modelPageUrl: value })).toThrow('Invalid model provider link');
  for (const value of [0, -1, Infinity, 128 * 1024 ** 3 + 1])
    expect(() => decode({ sizeBytes: value })).toThrow('Invalid model offer');
  expect(() => decode({ providerId: '' })).toThrow('Invalid model discovery text');
  expect(() =>
    decodeDiscoveryPlan({ schemaVersion: 1, ...plan, suggestions: [item, item] }),
  ).toThrow('Duplicate model');
  expect(() =>
    decodeDiscoveryPlan({
      schemaVersion: 1,
      ...plan,
      suggestions: [{ ...item, offers: [offer, offer] }],
    }),
  ).toThrow('Invalid model provider offers');
  expect(() =>
    decodeDiscoveryPlan({ schemaVersion: 1, ...plan, suggestions: Array(9).fill(item) }),
  ).toThrow('Invalid model discovery collection');
});

test('uses exact reviewed acquisition identities and refreshes legal host choices only after publication', async () => {
  const plan = discoveryPlan();
  const requests: Array<{ path: string; body: unknown; method: string | undefined }> = [];
  const installed = jest.fn<() => Promise<void>>(async () => undefined);
  const fetch: ModelApiFetch = async (path, options) => {
    requests.push({ path, body: options?.body, method: options?.method });
    if (path.endsWith('/context'))
      return response({
        schemaVersion: 1,
        context: { artifactKind: 'upscale_models', familyId: null },
      });
    if (options?.method === 'DELETE') return response({});
    if (path.endsWith('/acquire'))
      return job({ status: 'installed', fileName: 'folder/portrait.pth' });
    return job({ schemaVersion: 1, ...plan });
  };
  const client = new ModelDiscoveryClient(fetch, installed);
  const signal = new AbortController().signal;
  await client.context('upscale_models', '', signal);
  expect(await client.discover('upscale_models', '', false, jest.fn(), signal)).toEqual(plan);
  expect(installed).not.toHaveBeenCalled();
  const identity = plan.suggestions[0]?.identity ?? '';
  expect(await client.acquire(plan.planId, identity, 'openmodeldb', jest.fn(), signal)).toBe(
    'folder/portrait.pth',
  );
  expect(installed).toHaveBeenCalledTimes(1);
  expect(requests[2]).toEqual({
    path: `/sugarcubes/v1/models/discovery/${plan.planId}/acquire`,
    body: JSON.stringify({ identity, providerId: 'openmodeldb' }),
    method: 'POST',
  });
  await client.release(plan.planId);
  expect(requests.at(-1)?.method).toBe('DELETE');
});

test('cancels retained planning jobs and never refreshes choices after failed publication evidence', async () => {
  const fetch = jest.fn<ModelApiFetch>(async () => job({}, 'running'));
  const installed = jest.fn<() => Promise<void>>(async () => undefined);
  const client = new ModelDiscoveryClient(fetch, installed);
  const controller = new AbortController();
  const discovery = client.discover(
    'upscale_models',
    '',
    false,
    () => controller.abort(),
    controller.signal,
  );
  await expect(discovery).rejects.toThrow('Cancelled');
  expect(fetch).toHaveBeenCalledWith(`/sugarcubes/v1/models/jobs/${'c'.repeat(32)}`, {
    method: 'DELETE',
  });
  fetch.mockResolvedValueOnce(job({ status: 'installed', fileName: '' }));
  await expect(
    client.acquire(
      'a'.repeat(32),
      'b'.repeat(64),
      'openmodeldb',
      jest.fn(),
      new AbortController().signal,
    ),
  ).rejects.toThrow('invalid results');
  expect(installed).not.toHaveBeenCalled();
});

test('keeps keys out of browser persistence and accepts only secret-free typed credential status', async () => {
  const status = {
    schemaVersion: 1,
    provider: 'civitai',
    configured: true,
    storageAvailable: true,
    storageReason: null,
  };
  const fetch = jest.fn<ModelApiFetch>(async () => response(status));
  const client = new CivitaiCredentialClient(fetch);
  const signal = new AbortController().signal;
  expect(await client.status(signal)).toEqual({
    configured: true,
    storageAvailable: true,
    storageReason: null,
  });
  await client.save('fixture-only-key', signal);
  expect(fetch.mock.calls[1]).toEqual([
    '/sugarcubes/v1/models/credentials/civitai',
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ apiKey: 'fixture-only-key' }),
      signal,
    },
  ]);
  expect(localStorage.length).toBe(0);
  expect(sessionStorage.length).toBe(0);
  fetch.mockResolvedValueOnce(response({ ...status, configured: 'true' }));
  await expect(client.status(signal)).rejects.toThrow('Invalid CivitAI credential status');
  fetch.mockResolvedValueOnce(response({ error: 'Secure persistence unavailable' }, false));
  await expect(client.save('fixture-only-key', signal)).rejects.toThrow(
    'Secure persistence unavailable',
  );
});
