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
/** Prove the page asks SugarCubes' pasted-post route and treats an unreachable backend as unavailable. */
import { jest } from '@jest/globals';
import {
  DanbooruRoutes,
  type DanbooruResponse,
} from '../../../frontend/comfyui/ui/danbooru/infrastructure/DanbooruRoutes.js';

const reply = (status: number, body: unknown): DanbooruResponse => ({
  ok: status === 200,
  status,
  json: async () => body,
});

test('a post link asks by id and a CDN link by md5, returning the backend’s reply', async () => {
  const fetchApi = jest
    .fn<(path: string, options?: RequestInit) => Promise<DanbooruResponse>>()
    .mockResolvedValue(reply(200, { status: 'found', post: { id: 1 } }));
  const routes = new DanbooruRoutes(fetchApi);
  const signal = new AbortController().signal;
  expect(await routes.pastedPost({ kind: 'post', id: '1' }, signal)).toEqual({
    status: 'found',
    post: { id: 1 },
  });
  await routes.pastedPost({ kind: 'cdn', md5: 'a'.repeat(32) }, signal);
  expect(fetchApi.mock.calls.map(([path]) => path)).toEqual([
    '/sugarcubes/v1/danbooru/pasted-post?id=1',
    `/sugarcubes/v1/danbooru/pasted-post?md5=${'a'.repeat(32)}`,
  ]);
});

test('a backend that fails or cannot be reached reports Danbooru unavailable', async () => {
  const signal = new AbortController().signal;
  const failing = new DanbooruRoutes(async () => reply(500, null));
  expect(await failing.pastedPost({ kind: 'post', id: '1' }, signal)).toEqual({
    status: 'unavailable',
  });
  const unreachable = new DanbooruRoutes(async () => {
    throw new TypeError('Failed to fetch');
  });
  expect(await unreachable.pastedPost({ kind: 'post', id: '1' }, signal)).toEqual({
    status: 'unavailable',
  });
});

test('a paste that is dropped cancels its request instead of reporting', async () => {
  const controller = new AbortController();
  const routes = new DanbooruRoutes(
    (_path, options) =>
      new Promise((_resolve, reject) =>
        options?.signal?.addEventListener('abort', () => reject(new Error('aborted'))),
      ),
  );
  const pending = routes.pastedPost({ kind: 'post', id: '1' }, controller.signal);
  controller.abort();
  await expect(pending).rejects.toThrow('aborted');
});
