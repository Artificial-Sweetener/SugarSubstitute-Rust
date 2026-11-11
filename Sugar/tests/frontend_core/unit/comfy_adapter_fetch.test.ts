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
/** Safeguard native transport receiver ownership at the Comfy host boundary. */
import { expect, jest, test } from '@jest/globals';
import { ComfyAdapter } from '../../../frontend/comfyui/ui/core/ComfyAdapter.js';

test('retains the native Window receiver when a service calls the exported fetch', () => {
  const original = globalThis.fetch;
  const nativeFetch = jest.fn<typeof fetch>(() => new Promise<Response>(() => {}));
  globalThis.fetch = nativeFetch;
  try {
    const service = { fetch: new ComfyAdapter().getFetch() };
    if (!service.fetch) throw new Error('Host fetch was not exported');
    void service.fetch('/extensions/SugarCubes/models.txt', {});
    expect(nativeFetch.mock.contexts).toEqual([window]);
    expect(nativeFetch).toHaveBeenCalledWith('/extensions/SugarCubes/models.txt', {});
  } finally {
    globalThis.fetch = original;
  }
});
