/** @jest-environment node */
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
/** Prove native insertion sequencing keeps network preparation outside the mutation and always releases material. */
import { jest } from '@jest/globals';
import {
  InsertCanvasImage,
  type CanvasInsertionPort,
} from '../../../integrations/photoshop/src/application/InsertCanvasImage.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
const request = {
  document: { id: '1', geometryRevision: 'geometry', width: 512, height: 384 },
  image: { format: 'png' as const, encoding: 'base64' as const, data: 'fixture-png' },
  layerName: 'Generated',
};
/** Record transaction boundaries without pretending that a mock proves Photoshop rendering. */
function fixture() {
  const events: string[] = [];
  const report = jest.fn<(error: unknown) => void>();
  const dispose = jest.fn(async () => {
    events.push('dispose');
  });
  const prepared = { key: 'owned-file', width: 768, height: 512, dispose };
  const port: CanvasInsertionPort = {
    prepare: async () => {
      events.push('prepare');
      return prepared;
    },
    insert: async () => {
      events.push('insert');
      return {
        status: 'inserted',
        documentId: '1',
        layerId: '2',
        left: -128,
        top: -64,
        width: 768,
        height: 512,
      };
    },
    report,
  };
  return { port, prepared, events, report, dispose, cancellation: new CanvasCancellation() };
}
test('prepares, inserts and cleans exactly once while retaining intrinsic image geometry', async () => {
  const f = fixture();
  expect(await new InsertCanvasImage(f.port).execute(request, f.cancellation)).toMatchObject({
    status: 'inserted',
    left: -128,
    top: -64,
    width: 768,
    height: 512,
  });
  expect(f.events).toEqual(['prepare', 'insert', 'dispose']);
});
test('cancellation before preparation touches no native document or file', async () => {
  const f = fixture();
  f.cancellation.cancel();
  expect(await new InsertCanvasImage(f.port).execute(request, f.cancellation)).toMatchObject({
    status: 'cancelled',
  });
  expect(f.events).toEqual([]);
});
test('cancellation during download releases its prepared file without starting a transaction', async () => {
  const f = fixture();
  f.port.prepare = async () => {
    f.cancellation.cancel();
    return f.prepared;
  };
  expect(await new InsertCanvasImage(f.port).execute(request, f.cancellation)).toMatchObject({
    status: 'cancelled',
  });
  expect(f.events).toEqual(['dispose']);
});
test('preserves unexpected native failure while releasing prepared files', async () => {
  const f = fixture();
  const error = new Error('Native failure');
  f.port.insert = async () => {
    throw error;
  };
  await expect(new InsertCanvasImage(f.port).execute(request, f.cancellation)).rejects.toBe(error);
  expect(f.dispose).toHaveBeenCalledTimes(1);
});
test('reports cleanup failure without hiding a successfully committed insertion', async () => {
  const f = fixture();
  const error = new Error('Temporary file cleanup failed');
  f.dispose.mockImplementation(async () => {
    throw error;
  });
  expect(await new InsertCanvasImage(f.port).execute(request, f.cancellation)).toMatchObject({
    status: 'inserted',
  });
  expect(f.report).toHaveBeenCalledWith(error);
});
