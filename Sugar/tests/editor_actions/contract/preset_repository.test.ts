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
/** Prove Comfy per-user storage preserves Substitute's document and fails closed. */
import { expect, test } from '@jest/globals';
import { ComfyPresetRepository } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/ComfyPresetRepository.js';

test('treats only a 404 as an empty library and rejects unsupported schemas', async () => {
  expect(
    await new ComfyPresetRepository(async () => ({
      ok: false,
      status: 404,
      json: async () => null,
    })).load(),
  ).toEqual([]);
  await expect(
    new ComfyPresetRepository(async () => ({
      ok: false,
      status: 403,
      json: async () => null,
    })).load(),
  ).rejects.toThrow();
  await expect(
    new ComfyPresetRepository(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ version: 2, presets: [] }),
    })).load(),
  ).rejects.toThrow();
});
test('writes the common schema under the current Comfy user without browser-local storage', async () => {
  let request: RequestInit | undefined;
  const repository = new ComfyPresetRepository(async (path, options) => {
    expect(path).toBe('/userdata/SugarCubes%2Fpresets.json');
    request = options;
    return { ok: true, status: 200, json: async () => null };
  });
  await repository.save([]);
  expect(request?.method).toBe('POST');
  expect(JSON.parse(String(request?.body))).toEqual({ version: 1, presets: [] });
});
