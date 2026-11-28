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
/** Verify project storage and the Comfy readiness response consumed by the editor companion. */
import { test, expect } from '../support/LiveComfy.js';
import type { ComfyProjectFiles } from '../../../frontend/comfyui/ui/projects/infrastructure/ComfyProjects.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { createConnection } from 'node:net';
import type { ComfyWildcardFiles } from '../../../frontend/comfyui/ui/wildcards/infrastructure/ComfyWildcardFiles.js';

test('project files preserve Unicode, bytes and move conflicts in native per-user storage', async ({
  loaded: page,
}, info) => {
  const outcome = await page.evaluate(
    async (browser) => {
      const path = '/extensions/SugarCubes/comfyui/ui/projects/infrastructure/ComfyProjects.js';
      const mod: { ComfyProjectFiles: typeof ComfyProjectFiles } = await import(path);
      const { api } = await import('/scripts/api.js');
      const files = new mod.ComfyProjectFiles((url, options) => api.fetchApi(url, options));
      const first = browser + '/原稿.sugar';
      const second = browser + '/renamed.sugar';
      const binary = browser + '/mask.bin';
      const sample = 'use "fixture/🍬"\n';
      await files.write(first, sample);
      await files.writeBytes(binary, new Blob([new Uint8Array([0, 128, 255])]));
      const listed = (await files.list()).map((file) => file.path);
      const moved = await files.move(first, second);
      await files.write(first, 'another script');
      const conflict = await files.move(first, second);
      const blob = await files.readBytes(binary);
      return {
        listed: listed.includes(first),
        moved,
        conflict,
        restored: await files.read(second),
        bytes: blob ? Array.from(new Uint8Array(await blob.arrayBuffer())) : null,
      };
    },
    info.project.name + '-' + info.retry.toString(),
  );
  expect(outcome).toEqual({
    listed: true,
    moved: 'moved',
    conflict: 'exists',
    restored: 'use "fixture/🍬"\n',
    bytes: [0, 128, 255],
  });
});

test('wildcard files preserve line breaks and distinguish create, rename conflict and deletion', async ({
  loaded: page,
}) => {
  const operations = await page.evaluate(async () => {
    const path = '/extensions/SugarCubes/comfyui/ui/wildcards/infrastructure/ComfyWildcardFiles.js';
    const module: { ComfyWildcardFiles: typeof ComfyWildcardFiles } = await import(path);
    const { api } = await import('/scripts/api.js');
    const files = new module.ComfyWildcardFiles((url, init) => api.fetchApi(url, init));
    const id = crypto.randomUUID();
    const first = id + '/flowers.txt',
      second = id + '/moved.txt';
    const created = await files.create(first, 'rose\n紫苑\n');
    const duplicate = await files.create(first, 'replacement');
    const moved = await files.rename(first, second);
    const saved = await files.read(second);
    await files.create(first, 'spare');
    const conflict = await files.rename(first, second);
    await files.delete(second);
    await files.delete(first);
    return { created, duplicate, moved, saved, conflict, deleted: await files.read(second) };
  });
  expect(operations).toEqual({
    created: 'created',
    duplicate: 'exists',
    moved: 'renamed',
    saved: 'rose\n紫苑\n',
    conflict: 'exists',
    deleted: null,
  });
});

test('GIMP companion accepts the real loopback readiness response within its byte and time limits', async ({
  baseURL,
}) => {
  if (!baseURL) throw new Error('Comfy origin missing');
  const endpoint = new URL(baseURL);
  const wire = await new Promise<Buffer>((resolve, reject) => {
    const socket = createConnection(Number(endpoint.port), endpoint.hostname);
    const chunks: Buffer[] = [];
    const deadline = setTimeout(
      () => socket.destroy(new Error('Readiness exceeded three seconds')),
      3000,
    );
    socket.on('connect', () =>
      socket.write(
        'GET /system_stats HTTP/1.0\r\nHost: ' + endpoint.host + '\r\nConnection: close\r\n\r\n',
      ),
    );
    socket.on('data', (chunk: Buffer) => chunks.push(chunk));
    socket.once('end', () => resolve(Buffer.concat(chunks)));
    socket.once('error', reject);
    socket.once('close', () => clearTimeout(deadline));
  });
  expect(wire.length).toBeLessThanOrEqual(65536);
  const [headers, body] = wire.toString('utf8').split('\r\n\r\n');
  expect(headers).toMatch(/^HTTP\/1\.[01] 200 /);
  const facts: unknown = JSON.parse(body ?? '');
  expect(isRecord(facts) && isRecord(facts.system) && Array.isArray(facts.devices)).toBe(true);
});
