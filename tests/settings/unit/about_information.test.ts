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
/** Verify runtime metadata and shipped resources use their distinct, bounded transports. */
import { afterAll, beforeAll, expect, jest, test } from '@jest/globals';
import { ComfyAboutInformation } from '../../../frontend/comfyui/ui/settings/ComfyAboutInformation.js';
import type { AboutRequest } from '../../../frontend/comfyui/ui/settings/ComfyAboutInformation.js';

const timeoutDescriptor = Object.getOwnPropertyDescriptor(AbortSignal, 'timeout');
const timeout = jest.fn<(milliseconds: number) => AbortSignal>(() => new AbortController().signal);
beforeAll(() => {
  Object.defineProperty(AbortSignal, 'timeout', { configurable: true, value: timeout });
});
afterAll(() => {
  if (timeoutDescriptor) Object.defineProperty(AbortSignal, 'timeout', timeoutDescriptor);
  else Reflect.deleteProperty(AbortSignal, 'timeout');
});

/** Model only the response contract consumed by the metadata adapter. */
function response(payload: unknown, ok = true): Response {
  return {
    ok,
    status: ok ? 200 : 503,
    json: async () => payload,
    text: async () => String(payload),
  } as Response;
}

test('read installed versions and load the license without routing static assets through /api', async () => {
  const request = jest
    .fn<AboutRequest>()
    .mockImplementation(async (path) =>
      response(
        path === '/sugarcubes/status'
          ? { sugarCubesVersion: '9.8.7' }
          : { system: { comfyui_version: '0.29.0' } },
      ),
    );
  const resource = jest.fn<AboutRequest>().mockResolvedValue(response('AGPL license text'));
  const source = new ComfyAboutInformation(request, resource);
  const info = await source.load();
  expect(info.components.map((component) => component.version)).toEqual([
    '9.8.7',
    '0.29.0',
    '3.1.3',
  ]);
  expect(info.warnings).toEqual([]);
  expect(await source.license()).toBe('AGPL license text');
  expect(resource.mock.calls[0]?.[0]).toMatch(/\/legal\/AGPL-3\.0\.txt$/);
  expect(request).toHaveBeenCalledTimes(2);
  expect(request.mock.calls[0]?.[1].signal).toBeDefined();
  expect(timeout).toHaveBeenCalledWith(5000);
});

test('retain available component versions when another source fails and reject missing licenses', async () => {
  const source = new ComfyAboutInformation(
    async (path) => {
      if (path === '/sugarcubes/status') throw new Error('offline');
      return response({ system: { comfyui_version: '0.29.0' } });
    },
    async () => response('', false),
  );
  const info = await source.load();
  expect(info.components[0]?.version).toBe('Unavailable');
  expect(info.components[1]?.version).toBe('0.29.0');
  expect(info.warnings.join()).toContain('offline');
  await expect(source.license()).rejects.toThrow('503');
});
