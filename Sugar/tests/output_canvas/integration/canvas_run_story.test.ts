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
/** Prove Run-scoped result handoff and following through real execution owners. */

import { WorkflowResultStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowResultStore.js';
import { decodeComfyMedia } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyMediaDecoder.js';
import { promptId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowRun } from '../../../frontend/comfyui/ui/execution/domain/WorkflowRun.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { canvasBatchGridCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasContentSelection.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

/** Drive three stage sources without DOM rendering or fabricated catalog transitions. */
function fixture() {
  const seed = canvasCatalogFixture();
  const sources = seed.entries.map((entry) => entry.source);
  const results = new WorkflowResultStore();
  results.synchronize([seed.result.context]);
  let key = 0;
  const views = new CanvasViewRegistry(() => String(++key));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const view = views.create(seed.sessionId).viewId;
  /** Read only the workflow's authoritative retained results. */
  function catalog() {
    const result = results.read(seed.result.context.sessionId);
    if (!result) throw new Error('Missing workflow result session.');
    return canvasOutputCatalog(result, sources);
  }
  /** Start a known submission while leaving previous pixels represented. */
  function run(order: number): WorkflowRun {
    const value: WorkflowRun = {
      session: seed.result.context,
      sources,
      order,
      promptId: promptId(String(order)),
      status: 'running',
    };
    results.updateRun(value);
    selection.reconcile(view, catalog());
    return value;
  }
  /** Publish one attributable stage preview through the production result boundary. */
  function preview(active: WorkflowRun, stage: number) {
    results.publishPreview({
      sessionId: active.session.sessionId,
      generation: active.session.generation,
      promptId: active.promptId,
      sourceId: sources[stage]!.id,
      runOrder: active.order,
      sequence: stage + 1,
      token: `frame:${active.order}:${stage}`,
      dimensions: null,
    });
    selection.reconcile(view, catalog());
  }
  /** Commit original final media with the same source identity as its preview. */
  function final(active: WorkflowRun, stage: number, batches = 1) {
    const media = decodeComfyMedia({
      images: Array.from({ length: batches }, (_, index) => ({
        filename: `${active.order}-${stage}${index ? `-${index}` : ''}.png`,
        type: 'output',
      })),
    });
    if (!media) throw new Error('Invalid test final.');
    results.retain({
      run: active,
      source: sources[stage]!,
      ...media,
      merge: false,
      delivery: `${active.order}-${stage}`,
    });
    selection.reconcile(view, catalog());
  }
  return { results, views, view, selection, sources, catalog, run, preview, final };
}

test('hands over one result set at first content and follows three previews into stable finals', () => {
  const f = fixture();
  const prior = f.run(1);
  for (let stage = 0; stage < 3; stage++) f.final(prior, stage);
  const before = f.views.read(f.view).selection;
  const next = f.run(2);
  expect(f.catalog().entries).toHaveLength(3);
  expect(f.views.read(f.view).selection).toEqual(before);
  for (let stage = 0; stage < 3; stage++) {
    f.preview(next, stage);
    expect(f.catalog().entries.map((entry) => entry.source.id)).toEqual(
      f.sources.slice(0, stage + 1).map((source) => source.id),
    );
    expect(f.views.read(f.view).selection?.sourceId).toBe(f.sources[stage]!.id);
    f.final(next, stage);
    expect(f.catalog().entries).toHaveLength(stage + 1);
    expect(f.selection.items(f.view, f.catalog()).primary?.provenance.runOrder).toBe(2);
  }
  f.results.updateRun({ ...next, status: 'complete' });
  expect(f.catalog().entries).toHaveLength(3);
});

test('holds a user-selected stage within a Run and rearms only when the next Run has content', () => {
  const f = fixture();
  const first = f.run(1);
  f.final(first, 0);
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-source',
    selection: { sourceId: f.sources[0]!.id, batchIndex: 0 },
  });
  f.preview(first, 1);
  f.final(first, 1);
  f.preview(first, 2);
  expect(f.views.read(f.view)).toMatchObject({
    follow: 'manual',
    selection: { sourceId: f.sources[0]!.id },
  });
  const next = f.run(2);
  expect(f.views.read(f.view).follow).toBe('manual');
  f.preview(next, 0);
  expect(f.views.read(f.view).follow).toBe('auto');
  f.final(next, 0);
  f.preview(next, 1);
  expect(f.views.read(f.view).selection?.sourceId).toBe(f.sources[1]!.id);
});

test('does not replace prior results when a Run fails before producing any content', () => {
  const f = fixture();
  f.final(f.run(1), 0);
  const next = f.run(2);
  f.results.updateRun({ ...next, status: 'error' });
  expect(f.catalog().entries[0]?.items[0]?.file.filename).toBe('1-0.png');
});

test('opens a new Run preview as one image after leaving manual inspection of an older batch', () => {
  const f = fixture();
  f.final(f.run(1), 0, 2);
  f.selection.apply(f.view, f.catalog(), { type: 'batch', index: 1 });
  const next = f.run(2);
  f.results.publishPreview({
    sessionId: next.session.sessionId,
    generation: next.session.generation,
    promptId: next.promptId,
    sourceId: f.sources[0]!.id,
    runOrder: next.order,
    sequence: 1,
    token: 'new-run-preview',
    dimensions: null,
  });
  const inspected = canvasOutputCatalog(
    f.catalog().result,
    f.sources,
    f.selection.inspection(f.view),
  );
  expect(inspected.entries[0]?.items).toHaveLength(2);
  f.selection.reconcile(f.view, inspected);
  expect(f.views.read(f.view)).toMatchObject({
    follow: 'auto',
    mode: 'image',
    sourcePreview: true,
    selection: { sourceId: f.sources[0]!.id, batchIndex: 0 },
  });
  const displayed = f.views.read(f.view);
  f.selection.reconcile(f.view, f.catalog());
  expect(f.views.read(f.view)).toBe(displayed);
  f.selection.apply(f.view, f.catalog(), { type: 'overview', enabled: true });
  f.selection.reconcile(f.view, f.catalog());
  expect(f.views.read(f.view)).toMatchObject({ mode: 'overview', follow: 'manual' });
});

test('holds already displayed media until the first generated visual without adding an input mode', () => {
  const f = fixture();
  const source = f.sources[0]!;
  const payload = decodeComfyMedia({ images: [{ filename: 'input.png', type: 'input' }] });
  if (!payload) throw new Error('Invalid test media.');
  f.results.restore(f.catalog().result.context, source, payload.payload, 'loaded-media');
  f.selection.reconcile(f.view, f.catalog());
  expect(f.catalog().entries).toHaveLength(1);
  const active = f.run(1);
  expect(f.selection.items(f.view, f.catalog()).primary?.file.filename).toBe('input.png');
  f.preview(active, 1);
  expect(f.catalog().entries.map((entry) => entry.source.id)).toEqual([f.sources[1]!.id]);
  expect(f.views.read(f.view).selection?.sourceId).toBe(f.sources[1]!.id);
  f.final(active, 1);
  expect(f.selection.items(f.view, f.catalog()).primary?.file.filename).toBe('1-1.png');
});

test('never resurrects previous outputs after a new preview fails before its first final', () => {
  const f = fixture();
  f.final(f.run(1), 0);
  const next = f.run(2);
  f.preview(next, 1);
  f.results.updateRun({ ...next, status: 'error' });
  expect(f.catalog().entries).toHaveLength(0);
  expect(f.catalog().result.presentedRunOrder).toBe(2);
});

test('independent views share results while retaining their own manual navigation', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0);
  const other = f.views.create(f.catalog().sessionId).viewId;
  f.selection.reconcile(other, f.catalog());
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-source',
    selection: { sourceId: f.sources[0]!.id, batchIndex: 0 },
  });
  f.preview(active, 1);
  f.selection.reconcile(other, f.catalog());
  expect(f.views.read(f.view).selection?.sourceId).toBe(f.sources[0]!.id);
  expect(f.views.read(other).selection?.sourceId).toBe(f.sources[1]!.id);
});

test('automatically presents the progressive batch and replaces only presentable downstream positions', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0, 2);
  expect(f.views.read(f.view)).toMatchObject({ mode: 'overview', follow: 'auto' });
  f.preview(active, 1);
  const grid = canvasBatchGridCatalog(f.catalog(), f.views.read(f.view).selection, true)
    .entries[0]!;
  expect(f.views.read(f.view)).toMatchObject({
    mode: 'overview',
    selection: { sourceId: f.sources[1]!.id },
  });
  expect(grid.preview?.sourceId).toBe(f.sources[1]!.id);
  expect(grid.items.map((item) => [item.provenance.sourceId, item.batchIndex])).toEqual([
    [f.sources[0]!.id, 1],
  ]);
  f.final(active, 1, 2);
  const final = canvasBatchGridCatalog(f.catalog(), f.views.read(f.view).selection, true)
    .entries[0]!;
  expect(final.preview).toBeNull();
  expect(final.items.map((item) => item.provenance.sourceId)).toEqual([
    f.sources[1]!.id,
    f.sources[1]!.id,
  ]);
  f.selection.apply(f.view, f.catalog(), { type: 'batch', index: 1 });
  f.preview(active, 2);
  expect(f.views.read(f.view)).toMatchObject({
    mode: 'image',
    follow: 'manual',
    selection: { sourceId: f.sources[1]!.id, batchIndex: 1 },
  });
});

test('refuses a preview-only source that lacks the exact manually selected batch position', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0, 3);
  f.preview(active, 1);
  f.selection.apply(f.view, f.catalog(), {
    type: 'select',
    selection: { sourceId: f.sources[0]!.id, batchIndex: 2 },
  });
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-source',
    selection: { sourceId: f.sources[1]!.id, batchIndex: 2 },
  });
  expect(f.views.read(f.view)).toMatchObject({
    follow: 'manual',
    selection: { sourceId: f.sources[0]!.id, batchIndex: 2 },
  });
  f.preview(active, 1);
  expect(f.selection.preview(f.view, f.catalog())).toBeNull();
  expect(f.selection.items(f.view, f.catalog()).primary?.file.filename).toBe('1-0-2.png');
  f.final(active, 1, 3);
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-source',
    selection: { sourceId: f.sources[1]!.id, batchIndex: 2 },
  });
  expect(f.selection.items(f.view, f.catalog()).primary?.file.filename).toBe('1-1-2.png');
});

test('admits a source preview only in its first position or a source overview', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0, 3);
  f.preview(active, 1);
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-source',
    selection: { sourceId: f.sources[1]!.id, batchIndex: 0 },
  });
  expect(f.selection.preview(f.view, f.catalog())?.sourceId).toBe(f.sources[1]!.id);
  f.views.update(f.view, (state) => ({
    ...state,
    mode: 'image',
    selection: { sourceId: f.sources[1]!.id, batchIndex: 2 },
  }));
  expect(f.selection.preview(f.view, f.catalog())).toBeNull();
});

test('opens the live first batch position from a progressive overview', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0, 3);
  f.preview(active, 1);
  f.selection.apply(f.view, f.catalog(), { type: 'batch', index: 0 });
  expect(f.views.read(f.view)).toMatchObject({
    mode: 'image',
    follow: 'manual',
    sourcePreview: true,
    selection: { sourceId: f.sources[1]!.id, batchIndex: 0 },
  });
  expect(f.selection.preview(f.view, f.catalog())?.sourceId).toBe(f.sources[1]!.id);
});

test('opens an explicit preview without changing source-tab overview navigation', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0, 3);
  f.preview(active, 1);
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-preview',
    sourceId: f.sources[1]!.id,
  });
  expect(f.views.read(f.view)).toMatchObject({
    mode: 'image',
    follow: 'manual',
    sourcePreview: true,
  });
  f.preview(active, 1);
  expect(f.views.read(f.view).mode).toBe('image');
  f.final(active, 1, 3);
  f.selection.apply(f.view, f.catalog(), { type: 'batch', index: 2 });
  f.selection.apply(f.view, f.catalog(), {
    type: 'select-preview',
    sourceId: f.sources[1]!.id,
  });
  expect(f.views.read(f.view).selection?.batchIndex).toBe(2);
  expect(f.selection.preview(f.view, f.catalog())).toBeNull();
});

test('keeps the source overview available while its first position is a live preview', () => {
  const f = fixture();
  const active = f.run(1);
  f.final(active, 0, 3);
  f.preview(active, 1);
  expect(f.catalog().entries[1]?.summary.batchCount).toBe(1);
  f.selection.apply(f.view, f.catalog(), { type: 'select-preview', sourceId: f.sources[1]!.id });
  f.selection.apply(f.view, f.catalog(), { type: 'overview', enabled: true });
  expect(f.views.read(f.view)).toMatchObject({ mode: 'overview', sourcePreview: true });
  expect(f.selection.preview(f.view, f.catalog())?.sourceId).toBe(f.sources[1]!.id);
});
