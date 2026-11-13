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
import { nativeCanvasChanges } from '../../../integrations/shared/CanvasChangeRelay.js';

/** Settle coalesced native request and publication microtasks. */
async function settled(): Promise<void> {
  for (let n = 0; n < 40; n++) await Promise.resolve();
}

/** Retain only the external canvas boundary while exercising real preview and capture owners. */
function fixture(observesChanges = true) {
  const document = { id: 'doc', geometryRevision: 'geometry', width: 2, height: 1 };
  const changes = new Set<(change: CanvasSourceChange) => void>();
  let revision = 1;
  let hold: Promise<void> | null = null;
  const execute = jest.fn(async (query: CanvasSourceQuery): Promise<CanvasSourceResult> => {
    if (query.type === 'context')
      return {
        observesChanges,
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
    changes: observesChanges
      ? nativeCanvasChanges((listener) => {
          changes.add(listener);
          return () => {
            changes.delete(listener);
          };
        })
      : null,
  };
  const report = jest.fn();
  // The loaded workflow, and the workflow whose documents the canvas holds.
  const workflow = { current: null as string | null, held: null as string | null };
  const store = new CanvasSourcePreviews(new CanvasSourceCapture(() => host), () => host, report, {
    loaded: () => workflow.current,
    holds: (_canvasId: string, observed: string | null) => observed === workflow.held,
  });
  const source: Exclude<CanvasInputSource, { kind: 'file' }> = {
    kind: 'canvas',
    canvasId: 'editor',
    documentId: null,
    target: { kind: 'selection' },
  };
  return {
    store,
    workflow,
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

test('refreshes capture-only previews explicitly and fences a superseded in-flight capture', async () => {
  const f = fixture(false),
    changed = jest.fn<(value: CanvasSourcePreview) => void>();
  const release = f.hold();
  const stop = f.store.observe(f.source, changed);
  await settled();
  f.changed();
  f.store.refresh('editor');
  release();
  await settled();
  expect(f.changes.size).toBe(0);
  expect(changed.mock.calls.map(([value]) => value.kind)).toEqual(['loading', 'pixels']);
  expect(changed.mock.calls[1]?.[0]).toMatchObject({ revision: '2' });
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(2);
  await settled();
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(2);
  stop();
  f.store.dispose();
});

test('refreshes only the picker sources while unrelated mounted previews retain their state', async () => {
  const f = fixture(false);
  const first = jest.fn<(value: CanvasSourcePreview) => void>();
  const other = jest.fn<(value: CanvasSourcePreview) => void>();
  const stop = f.store.observe(f.source, first);
  const stopOther = f.store.observe({ ...f.source, documentId: 'doc' }, other);
  await settled();
  first.mockClear();
  other.mockClear();
  f.execute.mockClear();
  f.changed();
  f.store.refreshSources([f.source]);
  await settled();
  expect(first).toHaveBeenCalledTimes(1);
  expect(other).not.toHaveBeenCalled();
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(1);
  stop();
  stopOther();
  f.store.dispose();
});

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

test('a workflow switch recaptures mounted previews and keys later ones by the new workflow', async () => {
  const f = fixture(false);
  f.workflow.current = f.workflow.held = 'workflow-a';
  const before = jest.fn<(value: CanvasSourcePreview) => void>();
  const stopBefore = f.store.observe(f.source, before);
  await settled();
  expect(before.mock.calls.at(-1)?.[0]).toMatchObject({ kind: 'pixels', revision: '1' });
  // The next workflow's canvas numbers its documents afresh, so the same source names new pixels.
  f.changed();
  f.workflow.current = f.workflow.held = 'workflow-b';
  const after = jest.fn<(value: CanvasSourcePreview) => void>();
  const stopAfter = f.store.observe(f.source, after);
  expect(after.mock.calls[0]?.[0]).toEqual({ kind: 'loading' });
  f.store.workflowChanged();
  await settled();
  const revisions = (listener: typeof after) =>
    listener.mock.calls.flatMap(([value]) => (value.kind === 'pixels' ? [value.revision] : []));
  expect(revisions(after)).toEqual(['2']);
  expect(after.mock.calls.at(-1)?.[0]).toMatchObject({ kind: 'pixels', revision: '2' });
  // The earlier workflow's preview is no longer held, so it stays loading.
  expect(revisions(before)).toEqual(['1']);
  expect(before.mock.calls.at(-1)?.[0]).toEqual({ kind: 'loading' });
  const captures = f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  f.store.workflowChanged();
  await settled();
  expect(f.execute.mock.calls.filter(([query]) => query.type === 'capture')).toHaveLength(captures);
  stopBefore();
  stopAfter();
  expect(f.report).not.toHaveBeenCalled();
});

test('a preview waits for the canvas to hold its workflow and drops captures from an earlier one', async () => {
  const f = fixture(false);
  const captures = () => f.execute.mock.calls.filter(([query]) => query.type === 'capture').length;
  f.workflow.current = 'workflow-b';
  f.workflow.held = 'workflow-a';
  const preview = jest.fn<(value: CanvasSourcePreview) => void>();
  const stop = f.store.observe(f.source, preview);
  await settled();
  // The canvas still holds the earlier workflow, whose documents share these identities.
  expect(captures()).toBe(0);
  expect(preview.mock.calls.map(([value]) => value.kind)).toEqual(['loading']);
  f.workflow.held = 'workflow-b';
  const release = f.hold();
  f.store.refresh('editor');
  await settled();
  expect(captures()).toBe(1);
  // The canvas moves to another workflow while that capture is in flight.
  f.workflow.held = 'workflow-c';
  release();
  await settled();
  expect(preview.mock.calls.map(([value]) => value.kind)).toEqual(['loading']);
  f.changed();
  f.workflow.held = 'workflow-b';
  f.store.refresh('editor');
  await settled();
  expect(preview.mock.calls.at(-1)?.[0]).toMatchObject({ kind: 'pixels', revision: '2' });
  stop();
  expect(f.report).not.toHaveBeenCalled();
});
