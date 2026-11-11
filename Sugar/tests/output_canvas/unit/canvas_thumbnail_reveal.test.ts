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
/** Prove exact thumbnail inspection is independent of rendering and cannot select superseded files. */
import { jest } from '@jest/globals';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import {
  canvasRevealTarget,
  resolveCanvasReveal,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasReveal.js';
import { CanvasRevealCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasRevealCoordinator.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';
import { WorkflowRunRegistry } from '../../../frontend/comfyui/ui/execution/application/WorkflowRunRegistry.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';

test('rejects old runs, workflow replacements, and revisions of the same batch position', () => {
  const catalog = canvasCatalogFixture();
  const item = catalog.entries[0]!.items[0]!;
  const target = canvasRevealTarget(item);
  expect(resolveCanvasReveal(catalog, target)).toBe(item);
  for (const changed of [
    canvasCatalogFixture(2),
    canvasCatalogFixture(1, 1),
    canvasCatalogFixture(1, 0, undefined, 'other'),
    canvasCatalogFixture(1, 0, undefined, 'canvas', 2),
  ])
    expect(resolveCanvasReveal(changed, target)).toBeNull();
});

test('selects a single exact final manually and exits grid and comparison together', () => {
  const catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'reveal');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  selection.apply(view, catalog, { type: 'overview', enabled: true });
  selection.apply(view, catalog, { type: 'comparison', enabled: true });
  expect(
    selection.apply(view, catalog, {
      type: 'reveal',
      target: canvasRevealTarget(catalog.entries[0]!.items[0]!),
    }),
  ).toBe(true);
  expect(views.read(view)).toMatchObject({
    mode: 'image',
    follow: 'manual',
    sourcePreview: false,
    comparison: null,
    selection: { sourceId: catalog.entries[0]!.source.id, batchIndex: 0 },
  });
});

test('does not open a destination for a stale thumbnail and fences synchronous workflow switches', async () => {
  const catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'reveal');
  const viewId = views.create(catalog.sessionId).viewId;
  let current = { viewId, catalog };
  const reveal = jest.fn(async () => true);
  const open = jest.fn(() => ({ reveal }));
  const coordinator = new CanvasRevealCoordinator({ read: () => current, open });
  expect(
    await coordinator.reveal(canvasRevealTarget(canvasCatalogFixture(2).entries[0]!.items[0]!)),
  ).toBe(false);
  expect(open).not.toHaveBeenCalled();
  expect(await coordinator.reveal(canvasRevealTarget(catalog.entries[0]!.items[0]!))).toBe(true);
  open.mockImplementation(() => {
    current = { viewId, catalog: canvasCatalogFixture(1, 1) };
    return { reveal };
  });
  reveal.mockClear();
  expect(await coordinator.reveal(canvasRevealTarget(catalog.entries[0]!.items[0]!))).toBe(false);
  expect(reveal).not.toHaveBeenCalled();
});

test('holds a revealed previous final when the already-running job delivers its first preview', () => {
  const catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'manual-reveal');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  const item = catalog.entries[0]!.items[0]!;
  const running: CanvasCatalogSnapshot = {
    ...catalog,
    result: { ...catalog.result, runOrder: 2, runStatus: 'running' },
  };
  selection.apply(view, running, { type: 'reveal', target: canvasRevealTarget(item) });
  const preview = {
    sessionId: item.provenance.sessionId,
    generation: 0,
    promptId: 'prompt:running' as const,
    sourceId: catalog.entries[1]!.source.id,
    runOrder: 2,
    sequence: 1,
    token: 'preview:running',
    dimensions: null,
  };
  const arrived: CanvasCatalogSnapshot = {
    ...running,
    entries: running.entries.map((entry, index) => (index === 1 ? { ...entry, preview } : entry)),
    result: { ...running.result, preview },
  };
  selection.reconcile(view, arrived);
  expect(views.read(view)).toMatchObject({
    follow: 'manual',
    sourcePreview: false,
    selection: { sourceId: item.provenance.sourceId, batchIndex: 0 },
  });
  expect(selection.preview(view, arrived)).toBeNull();
  selection.reconcile(view, canvasCatalogFixture(3));
  expect(views.read(view).follow).toBe('auto');
});

test('holds explicit inspection through a reserved submission before its acceptance receipt arrives', () => {
  const catalog = canvasCatalogFixture();
  const submissions = new WorkflowRunRegistry();
  const context = {
    session: catalog.result.context,
    sources: catalog.entries.map((entry) => entry.source),
  };
  submissions.reserve(context);
  submissions.reserve(context);
  const views = new CanvasViewRegistry(() => 'reserved-reveal');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, submissions);
  selection.reconcile(view, catalog);
  const item = catalog.entries[0]!.items[0]!;
  selection.apply(view, catalog, { type: 'reveal', target: canvasRevealTarget(item) });
  selection.reconcile(view, canvasCatalogFixture(2));
  expect(views.read(view)).toMatchObject({
    follow: 'manual',
    sourcePreview: false,
    selection: { sourceId: item.provenance.sourceId, batchIndex: 0 },
  });
  selection.reconcile(view, canvasCatalogFixture(3));
  expect(views.read(view).follow).toBe('auto');
});

test('keeps a completed thumbnail available for explicit inspection after the next preview hands over', () => {
  const prior = canvasCatalogFixture();
  const item = prior.entries[0]!.items[0]!;
  const preview = {
    sessionId: item.provenance.sessionId,
    generation: 0,
    promptId: 'prompt:next' as const,
    sourceId: prior.entries[1]!.source.id,
    runOrder: 2,
    sequence: 1,
    token: 'preview:next',
    dimensions: null,
  };
  const result = { ...prior.result, runOrder: 2, presentedRunOrder: 2, preview };
  const catalog = canvasOutputCatalog(
    result,
    prior.entries.map((entry) => entry.source),
  );
  expect(catalog.entries.flatMap((entry) => entry.items)).toHaveLength(0);
  expect(resolveCanvasReveal(catalog, canvasRevealTarget(item))).toBe(item);
  const inspected = canvasOutputCatalog(result, catalog.sources, {
    sourceId: item.provenance.sourceId,
    batchIndex: item.batchIndex,
  });
  expect(
    inspected.entries.find((entry) => entry.source.id === item.provenance.sourceId)?.items,
  ).toContain(item);
  expect(
    inspected.entries
      .flatMap((entry) => entry.items)
      .every((retained) => retained.provenance.sourceId === item.provenance.sourceId),
  ).toBe(true);
});
