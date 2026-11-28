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
/** Exercise native annotated input paths and the codecs behind image and mask editing. */
import { test, expect } from '../support/LiveComfy.js';
import UPNG from 'upng-js';
import { readFile } from 'node:fs/promises';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

for (const kind of ['image', 'mask'] as const) {
  test(`native ${kind} preview preserves the selected channel and dimensions after upload`, async ({
    request,
  }) => {
    const samples = new Uint8Array([10, 20, 30, 127, 40, 50, 60, 255]);
    const png = await readFile(new URL('../support/translucent-source.png', import.meta.url));
    const uploaded = await request.post('/upload/image', {
      multipart: {
        image: { name: 'host-preview.png', mimeType: 'image/png', buffer: Buffer.from(png) },
        type: 'input',
        overwrite: 'true',
      },
    });
    await expect(uploaded).toBeOK();
    const receipt: unknown = await uploaded.json();
    if (!isRecord(receipt) || typeof receipt.name !== 'string')
      throw new Error('Missing uploaded filename');
    const result = await request.post('/sugarcubes/v2/canvas-inputs/preview', {
      data: {
        file: receipt.name,
        maximumSize: 0,
        settings: {
          kind,
          classType: kind === 'image' ? 'LoadImage' : 'LoadImageMask',
          field: 'image',
          inputs: kind === 'image' ? {} : { channel: 'green' },
        },
      },
    });
    await expect(result).toBeOK();
    expect(result.headers()['content-type']).toContain('image/png');
    const decoded = UPNG.decode(Uint8Array.from(await result.body()).buffer);
    expect([decoded.width, decoded.height]).toEqual([2, 1]);
    expect(new Uint8Array(UPNG.toRGBA8(decoded)[0]!)).toEqual(
      kind === 'image' ? samples : new Uint8Array([20, 20, 20, 255, 50, 50, 50, 255]),
    );
  });
}
