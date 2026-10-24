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
/** Prove committed source updates coalesce without stale pixels or native subscriptions surviving their views. */
import { jest } from '@jest/globals';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourcePreviews.js';
import type { CanvasSourcePreview } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourcePreviews.js';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasSourceChange,
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasInputSource,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

/** Settle coalesced native request and publication microtasks. */
async function settled(): Promise<void> {
  for (let n = 0; n < 40; n++) await Promise.resolve();
}

/** Retain only the external canvas boundary while exercising real preview and capture owners. */
function fixture() {
  const document = { id: 'doc', geometryRevision: 'geometry', width: 2, height: 1 };
  const changes = new Set<(change: CanvasSourceChange) => void>();
  let revision = 1;
  let hold: Promise<void> | null = null;
  const execute = jest.fn(async (query: CanvasSourceQuery): Promise<CanvasSourceResult> => {
    if (query.type === 'context')
      return {
        kind: 'context',
        supported: true,
        document,
        revision: String(revision),
        targets: ['selection'],
        layers: [],
        currentLayerId: null,
      };
    const captured = revision;
    if (hold) await hold;
    return {
      kind: 'captured',
      snapshots: query.targets.map((target) => ({
        target,
        document,
        revision: String(captured),
        pixels: encodeCanvasRaster(2, 1, 'coverage8', Uint8Array.from([captured, 0])),
      })),
    };
  });
  const host: CanvasSourceHost = {
    canvasId: 'editor',
    execute,
    subscribe: (listener) => {
      changes.add(listener);
      return () => {
        changes.delete(listener);
      };
    },
  };
  const report = jest.fn();
  const store = new CanvasSourcePreviews(new CanvasSourceCapture(() => host), () => host, report);
  const source: Exclude<CanvasInputSource, { kind: 'file' }> = {
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' },
  };
  return {
    store,
    source,
    execute,
    report,
    changes,
    changed: () => {
      revision++;
      for (const listener of changes) listener({ documentId: 'doc', revision: String(revision) });
    },
    hold: () => {
      let release = () => {};
      hold = new Promise<void>((resolve) => {
        release = resolve;
      });
      return () => {
        hold = null;
        release();
      };
    },
  };
}

test('shares one native watch and capture and stops watching when the last consumer leaves', async () => {
  const f = fixture(),
    first = jest.fn<(value: CanvasSourcePreview) => void>(),
    second = jest.fn<(value: CanvasSourcePreview) => void>();
  const stop = f.store.observe(f.source, first),
    stop2 = f.store.observe(f.source, second);
  await settled();
  expect(f.changes.size).toBe(1);
  expect(f.execute).toHaveBeenCalledTimes(2);
  expect(first).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'pixels' }));
  f.changed();
  f.changed();
  await settled();
  expect(f.execute).toHaveBeenCalledTimes(4);
  stop();
  expect(f.changes.size).toBe(1);
  stop2();
  expect(f.changes.size).toBe(0);
  f.store.dispose();
  expect(f.report).not.toHaveBeenCalled();
});

test('drops obsolete captures when another committed edit arrives and only publishes the new revision', async () => {
  const f = fixture(),
    listener = jest.fn<(value: CanvasSourcePreview) => void>(),
    release = f.hold();
  const stop = f.store.observe(f.source, listener);
  await settled();
  expect(listener.mock.calls.map(([value]) => value.kind)).toEqual(['loading']);
  f.changed();
  release();
  await settled();
  expect(listener.mock.calls.map(([value]) => value.kind)).toEqual(['loading', 'pixels']);
  expect(listener.mock.calls[1]?.[0]).toMatchObject({ pixels: { data: 'AgA=' } });
  stop();
  f.store.dispose();
});

test('an unmounted source receives neither a late capture nor an unowned error toast', async () => {
  const f = fixture(),
    listener = jest.fn<(value: CanvasSourcePreview) => void>(),
    release = f.hold();
  const stop = f.store.observe(f.source, listener);
  await settled();
  stop();
  release();
  await settled();
  expect(listener).toHaveBeenCalledTimes(1);
  expect(f.changes.size).toBe(0);
  expect(f.report).not.toHaveBeenCalled();
  f.store.dispose();
});

test('keeps full-resolution interpretation sources distinct from display thumbnails with one native watch', async () => {
  const f = fixture(),
    bounded = jest.fn(),
    full = jest.fn();
  const stop = f.store.observe(f.source, bounded),
    stopFull = f.store.observe(f.source, full, 0);
  await settled();
  expect(
    f.execute.mock.calls
      .filter(([query]) => query.type === 'capture')
      .map(([query]) => (query.type === 'capture' ? query.maximumSize : -1)),
  ).toEqual([288, 0]);
  expect(f.changes.size).toBe(1);
  expect(full).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'pixels' }));
  stop();
  stopFull();
  f.store.dispose();
  expect(f.report).not.toHaveBeenCalled();
});

test('shares the largest mounted display capture and reuses it for smaller consumers', async () => {
  const f = fixture(),
    small = jest.fn(),
    large = jest.fn();
  const stopSmall = f.store.observe(f.source, small, 128);
  await settled();
  const stopLarge = f.store.observe(f.source, large, 512);
  await settled();
  expect(
    f.execute.mock.calls
      .filter(([query]) => query.type === 'capture')
      .map(([query]) => (query.type === 'capture' ? query.maximumSize : 0)),
  ).toEqual([128, 512]);
  const stopSmaller = f.store.observe(f.source, jest.fn(), 64);
  await settled();
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(2);
  expect(small).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'pixels' }));
  expect(large).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'pixels' }));
  expect(f.changes.size).toBe(1);
  stopSmaller();
  stopLarge();
  stopSmall();
  f.store.dispose();
  expect(f.report).not.toHaveBeenCalled();
});
