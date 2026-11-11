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
/** Prove latest workflow results preserve attribution, batch positions and stale-update boundaries. */

import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import {
  outputSourceId,
  promptId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowSessionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSession.js';
import type { WorkflowRun } from '../../../frontend/comfyui/ui/execution/domain/WorkflowRun.js';
import type { WorkflowOutputSource } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreview.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';

const a: WorkflowSessionContext = {
  sessionId: workflowSessionId('a'),
  title: 'same',
  path: 'same.json',
  generation: 0,
};
const b: WorkflowSessionContext = { ...a, sessionId: workflowSessionId('b') };
const source: WorkflowOutputSource = {
  id: outputSourceId('42:23'),
  executionPath: '42:23',
  label: 'Preview',
  breadcrumb: ['Nested'],
  order: 0,
};

/** Apply a validated event to the public owner rather than mirroring its internal storage layout. */
function retain(
  store: WorkflowResultStore,
  run: WorkflowRun,
  value: unknown,
  merge = false,
  expectedRevision?: number,
) {
  const decoded = decodeComfyMedia(value);
  if (!decoded) throw new Error('Invalid test event');
  return store.retain({
    run,
    source,
    ...decoded,
    merge,
    ...(expectedRevision !== undefined ? { expectedRevision } : {}),
  });
}

/** Declare a source-level transient frame for a known accepted job. */
function preview(run: WorkflowRun): WorkflowPreview {
  return {
    sessionId: run.session.sessionId,
    generation: run.session.generation,
    promptId: run.promptId,
    sourceId: source.id,
    runOrder: run.order,
    sequence: 1,
    token: 'frame',
    dimensions: null,
  };
}

test('rejects late source previews after its same-run final even while other nodes are running', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([a]);
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  expect(store.publishPreview(preview(run))).toBe(true);
  retain(store, run, { images: [{ filename: 'final.png' }] });
  expect(store.read(a.sessionId)?.preview).toBeNull();
  expect(store.read(a.sessionId)?.runStatus).toBe('running');
  expect(store.publishPreview({ ...preview(run), sequence: 2 })).toBe(false);
  expect(store.read(a.sessionId)?.preview).toBeNull();
  retain(store, run, { text: 'metadata-only replacement' });
  expect(store.read(a.sessionId)?.outputs[0]?.items).toEqual([]);
  expect(store.publishPreview({ ...preview(run), sequence: 3 })).toBe(false);
  const next = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('two'));
  expect(store.publishPreview(preview(next))).toBe(true);
});

test('retires a source preview only when its first batch member becomes final', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([a]);
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('partial'));
  expect(store.publishPreview(preview(run))).toBe(true);
  retain(store, run, { images: [null, { filename: 'second.png' }, { filename: 'third.png' }] });
  expect(store.read(a.sessionId)?.preview?.promptId).toBe(run.promptId);
  expect(store.publishPreview({ ...preview(run), sequence: 2 })).toBe(true);
  retain(store, run, {
    images: [{ filename: 'first.png' }, { filename: 'second.png' }, { filename: 'third.png' }],
  });
  expect(store.read(a.sessionId)?.preview).toBeNull();
  expect(store.publishPreview({ ...preview(run), sequence: 3 })).toBe(false);
});

test('carried input snapshot media retains its actual provenance through text and can receive a new-run preview', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([a]);
  const decoded = decodeComfyMedia({ images: [{ filename: 'input.png', type: 'input' }] })!;
  store.restore(a, source, decoded.payload, 'input-snapshot');
  const original = store.read(a.sessionId)?.outputs[0]?.items[0];
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  expect(store.publishPreview(preview(run))).toBe(true);
  retain(store, run, { images: [], text: ['metadata'] });
  expect(store.read(a.sessionId)?.outputs[0]?.items[0]).toBe(original);
  expect(store.read(a.sessionId)?.preview).toEqual(preview(run));
  expect(store.publishPreview({ ...preview(run), sequence: 2 })).toBe(true);
});

test('keeps identical paths and graph IDs separate between two workflows', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([a, b]);
  const one = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  const two = runs.accept(runs.reserve({ session: b, sources: [source] }), promptId('two'));
  retain(store, one, { images: [{ filename: 'a.png' }] });
  retain(store, two, { images: [{ filename: 'b.png' }] });
  const first = store.read(a.sessionId)?.outputs[0]?.items[0];
  const second = store.read(b.sessionId)?.outputs[0]?.items[0];
  expect(first?.file.filename).toBe('a.png');
  expect(second?.file.filename).toBe('b.png');
  expect(first?.id).not.toBe(second?.id);
});

test('merges declared incremental arrays, preserves unavailable positions, and replaces ordinary results', () => {
  const store = new WorkflowResultStore();
  store.synchronize([a]);
  const runs = new WorkflowRunRegistry();
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  expect(retain(store, run, { images: [{ filename: 'a.png' }, null], text: ['first'] })).toBe(true);
  expect(retain(store, run, { images: [{ filename: 'b.png' }], text: ['second'] }, true)).toBe(
    true,
  );
  let output = store.read(a.sessionId)?.outputs[0];
  expect(output?.items.map((item) => [item.batchIndex, item.file.filename])).toEqual([
    [0, 'a.png'],
    [2, 'b.png'],
  ]);
  expect(output?.batchCount).toBe(3);
  expect(output?.text).toEqual(['first', 'second']);
  expect(retain(store, run, { text: 'new text' })).toBe(true);
  output = store.read(a.sessionId)?.outputs[0];
  expect(output?.batchCount).toBe(0);
  expect(output?.text).toEqual(['new text']);
  expect(retain(store, run, { images: [{ filename: 'replacement.png' }] })).toBe(true);
  expect(store.read(a.sessionId)?.outputs[0]?.batchCount).toBe(1);
  expect(retain(store, run, { images: [{ filename: 'replacement.png' }] })).toBe(false);
});

test('preserves specifically input previews across text-only updates and workflow-snapshot restoration', () => {
  const store = new WorkflowResultStore();
  store.synchronize([a]);
  const decoded = decodeComfyMedia({ images: [{ filename: 'input.png', type: 'input' }] });
  if (!decoded) throw new Error('Missing input preview');
  store.restore(a, source, decoded.payload, 'snapshot');
  const runs = new WorkflowRunRegistry();
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  retain(store, run, { text: 'result', images: [] });
  expect(store.read(a.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('input.png');
});

test('distinguishes a merge from the same replace payload while deduplicating repeated merge delivery', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([a]);
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  const payload = { images: [{ filename: 'same.png' }] };
  expect(retain(store, run, payload)).toBe(true);
  expect(retain(store, run, payload, true)).toBe(true);
  expect(store.read(a.sessionId)?.outputs[0]?.batchCount).toBe(2);
  expect(retain(store, run, payload, true)).toBe(false);
});

test('rejects delayed older jobs and history that races a newer live revision', () => {
  const store = new WorkflowResultStore();
  store.synchronize([a]);
  const runs = new WorkflowRunRegistry();
  const old = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('old'));
  const current = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('new'));
  retain(store, old, { images: [{ filename: 'old.png' }] });
  const revision = store.sourceRevision(a.sessionId, source.id);
  retain(store, current, { images: [{ filename: 'new.png' }] });
  expect(retain(store, old, { images: [{ filename: 'delayed.png' }] })).toBe(false);
  expect(retain(store, current, { images: [{ filename: 'history.png' }] }, false, revision)).toBe(
    false,
  );
  expect(store.read(a.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('new.png');
});

test('cannot revive closed sessions or results from an obsolete graph generation', () => {
  const store = new WorkflowResultStore();
  store.synchronize([a]);
  const runs = new WorkflowRunRegistry();
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  store.synchronize([{ ...a, generation: 1 }]);
  expect(retain(store, run, { images: [{ filename: 'obsolete.png' }] })).toBe(false);
  store.synchronize([]);
  expect(retain(store, run, { images: [{ filename: 'closed.png' }] })).toBe(false);
  expect(store.read(a.sessionId)).toBeNull();
});

test('restores demonstrable workflow-owned snapshots with explicit non-job provenance', () => {
  const store = new WorkflowResultStore();
  store.synchronize([a]);
  const decoded = decodeComfyMedia({ images: [{ filename: 'restored.png' }] });
  if (!decoded) throw new Error('Missing decoded snapshot');
  expect(store.restore(a, source, decoded.payload, 'host-1')).toBe(true);
  const output = store.read(a.sessionId)?.outputs[0];
  expect(output?.provenance).toMatchObject({
    kind: 'workflow-snapshot',
    promptId: null,
    snapshotId: 'host-1',
  });
  expect(store.restore(a, source, decoded.payload, 'host-2')).toBe(false);
});

test('keeps previous finals during previews, releases preview metadata on final/terminal, and rejects late previews', () => {
  const store = new WorkflowResultStore();
  const runs = new WorkflowRunRegistry();
  store.synchronize([a]);
  const previous = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('one'));
  retain(store, previous, { images: [{ filename: 'final.png' }] });
  const run = runs.accept(runs.reserve({ session: a, sources: [source] }), promptId('two'));
  const preview = {
    sessionId: a.sessionId,
    generation: 0,
    promptId: run.promptId,
    sourceId: source.id,
    runOrder: run.order,
    sequence: 1,
    token: 'token',
    dimensions: null,
  };
  expect(store.publishPreview(preview)).toBe(true);
  expect(store.read(a.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('final.png');
  retain(store, run, { images: [{ filename: 'replacement.png' }] });
  expect(store.read(a.sessionId)?.preview).toBeNull();
  store.updateRun({ ...run, status: 'cancelled' });
  expect(store.publishPreview({ ...preview, sequence: 2 })).toBe(false);
  expect(store.read(a.sessionId)?.outputs[0]?.items[0]?.file.filename).toBe('replacement.png');
});

test('releases closed-job source metadata while keeping minimal late-event rejection identities', () => {
  const runs = new WorkflowRunRegistry();
  const id = promptId('one');
  runs.accept(runs.reserve({ session: a, sources: [source] }), id);
  runs.closeSession(a.sessionId);
  expect(runs.read(id)).toBeNull();
  expect(runs.isClosed(id)).toBe(true);
  expect(runs.outstanding()).toEqual([]);
  runs.retire(id);
  expect(runs.isClosed(id)).toBe(false);
});
