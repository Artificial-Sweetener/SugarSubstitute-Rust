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
/** Prove preview backpressure, exact crop framing and stale-result rejection independently of browser geometry. */
import { jest } from '@jest/globals';
import { InputMaskPreview } from '../../../frontend/comfyui/ui/input_canvas/application/InputMaskPreview.js';
import type { InputPreviewContentValue } from '../../../frontend/comfyui/ui/input_canvas/application/InputPreviewContent.js';
import type { MaskEditingPort } from '../../../frontend/comfyui/ui/input_canvas/application/MaskEditingPort.js';
import { DEFAULT_MASK_EDITS } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskEditSettings.js';
import {
  editMaskInput,
  type EditedMaskInput,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { maskPreviewPixels } from '../../../frontend/comfyui/ui/input_canvas/presentation/MaskPreviewWash.js';

/** Settle promise sequencing without timing-based retries or rendering refreshes. */
async function settled(): Promise<void> {
  for (let i = 0; i < 12; i++) await Promise.resolve();
}

/** Keep one nonrectangular mask inside an image with identifiable source pixels. */
function inputs() {
  const mask = encodeCanvasRaster(
    4,
    3,
    'coverage8',
    Uint8Array.from([0, 0, 0, 0, 0, 255, 64, 0, 0, 128, 0, 0]),
  );
  const image = encodeCanvasRaster(
    4,
    3,
    'rgba8',
    Uint8Array.from(Array.from({ length: 12 }, (_, i) => [i * 10, 80, 120, 255]).flat()),
  );
  return { mask, image };
}

test('retains context framing with an outside crop outline and shows only included pixels in actual-input mode', async () => {
  const { mask, image } = inputs(),
    published: InputPreviewContentValue[] = [];
  const prepare = jest.fn<MaskEditingPort['prepare']>(async (source, settings) =>
    editMaskInput(source, settings),
  );
  const port: MaskEditingPort = { prepare };
  const report = jest.fn(),
    preview = new InputMaskPreview(port, (value) => published.push(value), report);
  const settings = { ...DEFAULT_MASK_EDITS, crop_factor: 1 };
  preview.paint(mask, image, settings, 'context', 256, { width: 4, height: 3 });
  await settled();
  const context = published.at(-1);
  expect(context).toMatchObject({
    kind: 'pixels',
    outline: true,
    placement: { bounds: { left: 1, top: 1, width: 2, height: 2 } },
    frame: { width: 4, height: 3 },
  });
  if (context?.kind !== 'pixels') throw new Error('Missing context');
  const washed = maskPreviewPixels(context.pixels, context.background!, context.placement);
  expect([...washed.slice(0, 4)]).toEqual([0, 40, 60, 255]);
  expect([...washed.slice(5 * 4, 6 * 4)]).toEqual([153, 168, 188, 255]);
  preview.paint(mask, image, settings, 'input', 256, { width: 4, height: 3 });
  await settled();
  const actual = published.at(-1);
  expect(actual).toMatchObject({
    kind: 'pixels',
    frame: { width: 2, height: 2 },
    viewport: { bounds: { left: 1, top: 1, width: 2, height: 2 }, frame: { width: 4, height: 3 } },
  });
  if (actual?.kind !== 'pixels') throw new Error('Missing actual input');
  expect(actual.outline).toBe(false);
  expect(actual.background).toBe(image);
  expect(actual.pixels).toBe(context.pixels);
  expect(actual.placement).toBe(context.placement);
  expect(prepare).toHaveBeenCalledTimes(1);
  expect(report).not.toHaveBeenCalled();
  preview.dispose();
});

test('processes only the latest pending slider intent and never publishes an obsolete result', async () => {
  const { mask, image } = inputs(),
    published: InputPreviewContentValue[] = [];
  const requests: { feather: number; resolve: (value: EditedMaskInput) => void }[] = [];
  const port: MaskEditingPort = {
    prepare: (source, settings) =>
      new Promise((resolve) => requests.push({ feather: settings.feather, resolve })),
  };
  const preview = new InputMaskPreview(port, (value) => published.push(value), jest.fn());
  preview.paint(mask, image, { ...DEFAULT_MASK_EDITS, feather: 1 }, 'context', 256, null);
  preview.paint(mask, image, { ...DEFAULT_MASK_EDITS, feather: 2 }, 'context', 256, null);
  preview.paint(mask, image, { ...DEFAULT_MASK_EDITS, feather: 3 }, 'context', 256, null);
  expect(requests.map((item) => item.feather)).toEqual([1]);
  requests[0]!.resolve(editMaskInput(mask, { ...DEFAULT_MASK_EDITS, feather: 1 }));
  await settled();
  expect(published.filter((value) => value.kind === 'pixels')).toHaveLength(0);
  expect(requests.map((item) => item.feather)).toEqual([1, 3]);
  requests[1]!.resolve(editMaskInput(mask, { ...DEFAULT_MASK_EDITS, feather: 3 }));
  await settled();
  expect(published.filter((value) => value.kind === 'pixels')).toHaveLength(1);
  preview.dispose();
});
