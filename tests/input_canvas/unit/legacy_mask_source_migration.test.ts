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
/** Preserve actual saved private-mask pixels at the public workflow migration boundary. */
import { jest } from '@jest/globals';
import { LegacyMaskMigration } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/LegacyMaskMigration.js';
import type { CanvasMaskHost } from '../../../integrations/contracts/CanvasMasks.js';
import type {
  CanvasMaskCommand,
  CanvasMaskResult,
} from '../../../integrations/contracts/CanvasMasksProtocol.generated.js';
import type { CanvasPixelRaster } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeMaskPixels } from '../../../integrations/shared/CanvasMaskPixels.js';
import { decodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

const document = { id: 'old-document', geometryRevision: 'geometry', width: 2, height: 1 };
const mask = { canvasId: 'editor', document, id: 'private-channel' };
const properties = {
  sugarcubes_canvas_masks: {
    version: 1,
    field: 'image',
    files: ['unrelated-photo.png'],
    selected: 'first',
    entries: [
      { id: 'first', label: 'Authored region', file: 'unrelated-photo.png', mask },
      { id: 'file', label: 'Authored file', file: 'chosen.png', mask: null },
    ],
  },
};

test('migrates saved coverage into a real source image while preserving order and explicit files', async () => {
  const execute = jest.fn(
    async (command: CanvasMaskCommand): Promise<CanvasMaskResult> =>
      command.type === 'capture'
        ? {
            kind: 'captured',
            snapshots: command.masks.map((mask) => ({
              mask,
              revision: '1',
              pixels: encodeMaskPixels(2, 1, Uint8Array.from([64, 255])),
            })),
          }
        : { kind: 'failure', status: 'unsupported', message: 'Read only.' },
  );
  const host: CanvasMaskHost = { canvasId: 'editor', execute, subscribe: () => () => {} };
  const stage = jest.fn(async (_pixels: CanvasPixelRaster) => 'actual-region.png');
  const migration = new LegacyMaskMigration(() => host, { stage });
  const result = await migration.read(properties);
  expect(result).toMatchObject({
    version: 2,
    selected: 'first',
    entries: [
      { id: 'first', source: { kind: 'file', file: 'actual-region.png' } },
      { id: 'file', source: { kind: 'file', file: 'chosen.png' } },
    ],
  });
  const image = stage.mock.calls[0]?.[0];
  if (!image) throw new Error('Missing migrated source.');
  expect([...decodeCanvasRaster(image)]).toEqual([64, 64, 64, 191, 255, 255, 255, 0]);
  expect(execute).toHaveBeenCalledWith({ type: 'capture', masks: [mask], maximumSize: 0 });
  expect(await migration.read({ sugarcubes_canvas_masks: result })).toEqual(result);
});

test('fails closed for unavailable saved regions and corrupt identities', async () => {
  const migration = new LegacyMaskMigration(() => null, { stage: async () => 'unused.png' });
  await expect(migration.read(properties)).rejects.toThrow('Connect the canvas');
  await expect(
    migration.read({
      sugarcubes_canvas_masks: { ...properties.sugarcubes_canvas_masks, selected: 'not-an-entry' },
    }),
  ).rejects.toThrow('selected');
});
