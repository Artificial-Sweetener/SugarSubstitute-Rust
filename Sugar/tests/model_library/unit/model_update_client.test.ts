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
/** Prove shared update state remains server-owned and focus checks require durable consent. */
import { jest } from '@jest/globals';
import { ModelUpdateClient } from '../../../frontend/comfyui/ui/model_library/ModelUpdateClient.js';
import { ModelUpdateLifecycle } from '../../../frontend/comfyui/ui/model_library/ModelUpdateLifecycle.js';
import type { ModelApiFetch } from '../../../frontend/comfyui/ui/model_library/ModelApiTransport.js';
import type { ModelUpdatePort } from '../../../frontend/comfyui/ui/model_library/ModelUpdates.js';

/** Return the validated update envelope without involving live providers or model files. */
function response(enabled: boolean, proposals: unknown[] = []): Response {
  return { ok: true, json: async () => ({ schemaVersion: 1, enabled, proposals }) } as Response;
}

test('coalesces checks, publishes decisions, and rejects invalid identities before transport', async () => {
  const requests: string[] = [];
  let enabled = false;
  const fetch: ModelApiFetch = async (path, options) => {
    requests.push(path);
    if (options?.method === 'PUT') enabled = true;
    return response(enabled);
  };
  const client = new ModelUpdateClient(fetch);
  const observed = jest.fn();
  client.subscribe(observed);
  await client.load();
  expect(client.read().enabled).toBe(false);
  await client.setEnabled(true);
  await Promise.all([client.check(), client.check()]);
  expect(requests.filter((path) => path.endsWith('/check'))).toHaveLength(1);
  await client.dismiss('a'.repeat(64));
  await client.disableModel('a'.repeat(64));
  expect(requests.at(-1)).toBe(`/sugarcubes/v1/models/${'a'.repeat(64)}/updates/disable`);
  await expect(client.dismiss('../unsafe')).rejects.toThrow('Invalid model update identity');
  expect(observed).toHaveBeenCalledTimes(5);
});

test('coalesces selected-model checks and preserves offers when a refresh fails', async () => {
  const requests: string[] = [];
  let offline = false;
  const offer = {
    modelId: 'a'.repeat(64),
    sha256: 'b'.repeat(64),
    modelName: 'Fixture',
    versionId: '21',
  };
  const client = new ModelUpdateClient(async (path) => {
    requests.push(path);
    if (offline) throw new Error('offline');
    return response(true, [offer]);
  });
  await Promise.all([client.checkSelection(offer.modelId), client.checkSelection(offer.modelId)]);
  expect(requests).toEqual([`/sugarcubes/v1/models/${offer.modelId}/updates/check`]);
  offline = true;
  await expect(client.checkSelection(offer.modelId)).rejects.toThrow('offline');
  expect(client.read().proposals).toEqual([offer]);
  await expect(client.checkSelection('../unsafe')).rejects.toThrow('Invalid model update identity');
  expect(requests).toHaveLength(2);
});
test('starts and stops the reference focus behavior without defaulting consent to true', async () => {
  let enabled = false;
  const requests: string[] = [];
  const client = new ModelUpdateClient(async (path) => {
    requests.push(path);
    return response(enabled);
  });
  const errors = jest.fn();
  const focus = new ModelUpdateLifecycle(window, client, errors);
  focus.start();
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  window.dispatchEvent(new Event('focus'));
  expect(requests).toEqual(['/sugarcubes/v1/models/updates']);
  enabled = true;
  await client.load();
  window.dispatchEvent(new Event('focus'));
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(requests.at(-1)).toBe('/sugarcubes/v1/models/updates');
  const count = requests.length;
  focus.dispose();
  window.dispatchEvent(new Event('focus'));
  expect(requests).toHaveLength(count);
  expect(errors).not.toHaveBeenCalled();
});

test('resumes cached-page focus checks only after reloading durable consent', async () => {
  let enabled = true;
  const requests: string[] = [];
  const client = new ModelUpdateClient(async (path) => {
    requests.push(path);
    return response(enabled);
  });
  const focus = new ModelUpdateLifecycle(window, client, jest.fn());
  try {
    focus.start();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(requests.at(-1)).toBe('/sugarcubes/v1/models/updates');
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    const count = requests.length;
    window.dispatchEvent(new Event('focus'));
    expect(requests).toHaveLength(count);
    enabled = false;
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    window.dispatchEvent(new Event('focus'));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(requests.slice(count)).toEqual(['/sugarcubes/v1/models/updates']);
    window.dispatchEvent(new Event('focus'));
    expect(requests.slice(count)).toEqual(['/sugarcubes/v1/models/updates']);
    enabled = true;
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(requests.slice(count)).toEqual([
      '/sugarcubes/v1/models/updates',
      '/sugarcubes/v1/models/updates',
      '/sugarcubes/v1/models/updates',
    ]);
  } finally {
    focus.dispose();
  }
});

test('drains a suspended consent read before accepting restored-page consent', async () => {
  let finishFirst: () => void = () => {
    throw new Error('Consent read has not started');
  };
  let enabled = false;
  const load = jest.fn<ModelUpdatePort['load']>();
  load.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finishFirst = () => {
          enabled = true;
          resolve();
        };
      }),
  );
  load.mockImplementationOnce(async () => {
    enabled = false;
  });
  const check = jest.fn<ModelUpdatePort['check']>(async () => undefined);
  const errors = jest.fn();
  const updates: ModelUpdatePort = {
    read: () => ({ enabled, proposals: [] }),
    subscribe: () => () => undefined,
    load,
    check,
    checkSelection: async () => undefined,
    setEnabled: async () => undefined,
    dismiss: async () => undefined,
    disableModel: async () => undefined,
  };
  const focus = new ModelUpdateLifecycle(window, updates, errors);
  try {
    focus.start();
    await Promise.resolve();
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    window.dispatchEvent(new Event('focus'));
    expect(load).toHaveBeenCalledTimes(1);
    expect(check).not.toHaveBeenCalled();
    finishFirst();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(load).toHaveBeenCalledTimes(2);
    expect(enabled).toBe(false);
    expect(check).not.toHaveBeenCalled();
    expect(errors).not.toHaveBeenCalled();
  } finally {
    focus.dispose();
  }
});

test('keeps failed restored consent non-actionable and reports its original diagnostic', async () => {
  let fail = false;
  const failure = new TypeError('Consent transport failed');
  const requests: string[] = [];
  const client = new ModelUpdateClient(async (path) => {
    requests.push(path);
    if (fail) throw failure;
    return response(true);
  });
  const errors = jest.fn();
  const focus = new ModelUpdateLifecycle(window, client, errors);
  try {
    focus.start();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    const count = requests.length;
    fail = true;
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    window.dispatchEvent(new Event('focus'));
    expect(requests.slice(count)).toEqual(['/sugarcubes/v1/models/updates']);
    expect(errors).toHaveBeenCalledWith(failure);
    fail = false;
    window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }));
    window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }));
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(requests.at(-1)).toBe('/sugarcubes/v1/models/updates');
  } finally {
    focus.dispose();
  }
});
