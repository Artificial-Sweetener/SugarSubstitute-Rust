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
/** Prove source membership, exact navigation, execution following, and stable comparison revisions. */

import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { canvasSelectedItem } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasContentSelection.js';
import { canvasComparisonPair } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasComparisonSelection.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import { windowId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { canvasCatalogFixture, canvasSources } from '../support/CanvasCatalogFixture.js';

/** Create independent authoritative views around a stable result catalog. */
function fixture() {
  let identity = 0;
  const views = new CanvasViewRegistry(() => String(++identity));
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  const catalog = canvasCatalogFixture();
  const view = views.create(catalog.sessionId).viewId;
  selection.reconcile(view, catalog);
  return { views, selection, catalog, view };
}

describe('canvas workflow navigation', () => {
  test('compares first and last stages in the chosen batch rather than an arbitrary other image', () => {
    const catalog = canvasCatalogFixture();
    const result = {
      ...catalog.result,
      outputs: catalog.result.outputs.map((output) => ({
        ...output,
        items: output.items.flatMap((item) => [
          item,
          { ...item, id: `${item.id}:batch1` as const, batchIndex: 1 },
        ]),
      })),
    };
    const pair = canvasComparisonPair(canvasOutputCatalog(result, canvasSources()), 1);
    expect(pair?.primary?.provenance.sourceId).toBe(canvasSources()[0]?.id);
    expect(pair?.secondary?.provenance.sourceId).toBe(canvasSources()[2]?.id);
    expect(pair?.primary?.batchIndex).toBe(1);
    expect(pair?.secondary?.batchIndex).toBe(1);
  });

  test('settles manually selected source previews onto a real final without changing explicit batch intent', () => {
    const f = fixture();
    const source = f.catalog.entries[0]!.source;
    const preview = {
      sessionId: f.catalog.result.context.sessionId,
      generation: 0,
      promptId: 'prompt:next' as const,
      sourceId: source.id,
      runOrder: 2,
      sequence: 1,
      token: 'frame',
      dimensions: null,
    };
    const current = canvasOutputCatalog(
      { ...f.catalog.result, runOrder: 2, preview },
      canvasSources(),
    );
    f.selection.apply(f.view, current, {
      type: 'select-source',
      selection: { sourceId: source.id, batchIndex: 0 },
    });
    const final = canvasCatalogFixture(2, 0, [source]);
    f.selection.reconcile(f.view, final);
    expect(f.views.read(f.view)).toMatchObject({ follow: 'manual', selection: { batchIndex: 0 } });
    f.selection.apply(f.view, final, { type: 'batch', index: 2 });
    f.selection.reconcile(f.view, final);
    expect(f.views.read(f.view).selection?.batchIndex).toBe(0);
  });
  test('admits preview metadata only for source intent within a current non-comparison view', () => {
    const f = fixture();
    const source = f.catalog.entries[0]!.source;
    const preview = {
      sessionId: f.catalog.result.context.sessionId,
      generation: 0,
      promptId: 'prompt:two' as const,
      sourceId: source.id,
      runOrder: 2,
      sequence: 1,
      token: 'frame',
      dimensions: null,
    };
    const catalog = canvasOutputCatalog(
      { ...f.catalog.result, runOrder: 2, runStatus: 'running', preview },
      canvasSources(),
    );
    f.selection.reconcile(f.view, catalog);
    expect(f.selection.preview(f.view, catalog)).toBe(preview);
    f.selection.apply(f.view, catalog, {
      type: 'select',
      selection: { sourceId: source.id, batchIndex: 0 },
    });
    expect(f.selection.preview(f.view, catalog)).toBeNull();
    f.selection.apply(f.view, catalog, {
      type: 'select-source',
      selection: { sourceId: source.id, batchIndex: 2 },
    });
    expect(f.selection.preview(f.view, catalog)).toBeNull();
    f.selection.apply(f.view, catalog, {
      type: 'select-source',
      selection: { sourceId: source.id, batchIndex: 0 },
    });
    expect(f.selection.preview(f.view, catalog)).toBe(preview);
    const deleted = canvasOutputCatalog(catalog.result, []);
    expect(f.selection.preview(f.view, deleted)).toBeNull();
    expect(f.selection.preview(f.view, { ...catalog, generation: 1 })).toBeNull();
    f.selection.apply(f.view, catalog, { type: 'comparison', enabled: true });
    expect(f.views.read(f.view).comparison).toBeNull();
    expect(f.selection.preview(f.view, catalog)).toBe(preview);
    f.selection.apply(f.view, catalog, { type: 'comparison', enabled: false });
    f.selection.apply(f.view, catalog, { type: 'overview', enabled: true });
    expect(f.views.read(f.view).mode).toBe('overview');
    expect(f.selection.preview(f.view, catalog)).toBe(preview);
  });
  test('distinguishes source preview interest from an explicitly requested batch, including batch zero', () => {
    const f = fixture();
    expect(f.views.read(f.view)).toMatchObject({ sourcePreview: true });
    f.selection.manual(f.view);
    expect(f.views.read(f.view)).toMatchObject({ sourcePreview: true });
    f.selection.apply(f.view, f.catalog, { type: 'batch', index: 0 });
    expect(f.views.read(f.view)).toMatchObject({ sourcePreview: false });
    const source = f.catalog.entries[0]!.source;
    f.selection.apply(f.view, f.catalog, {
      type: 'select-source',
      selection: { sourceId: source.id, batchIndex: 0 },
    });
    expect(f.views.read(f.view)).toMatchObject({ sourcePreview: true, follow: 'manual' });
    expect(f.views.read(f.view).selection?.batchIndex).toBe(0);
    f.selection.apply(f.view, f.catalog, {
      type: 'select',
      selection: { sourceId: source.id, batchIndex: 0 },
    });
    expect(f.views.read(f.view)).toMatchObject({ sourcePreview: false });
    f.selection.apply(f.view, f.catalog, { type: 'follow', mode: 'auto' });
    expect(f.views.read(f.view)).toMatchObject({ sourcePreview: true, follow: 'auto' });
  });
  test('labels carried media by its actual run rather than the source latest text revision', () => {
    const catalog = canvasCatalogFixture();
    const result = {
      ...catalog.result,
      outputs: catalog.result.outputs.map((output) => ({
        ...output,
        items: output.items.map((item) => ({
          ...item,
          provenance: { ...item.provenance, runOrder: 0 },
        })),
      })),
    };
    expect(
      canvasOutputCatalog(result, canvasSources()).entries.every(
        (entry) => entry.summary.previousRun,
      ),
    ).toBe(true);
  });
  test('Auto deliberately chooses a valid remaining source when its current source is deleted', () => {
    const f = fixture();
    const catalog = canvasOutputCatalog(f.catalog.result, canvasSources().slice(0, 1));
    f.selection.reconcile(f.view, catalog);
    expect(f.views.read(f.view).selection?.sourceId).toBe(catalog.entries[0]?.source.id);
  });
  test('does not republish unchanged navigation for every frame of the selected source preview', () => {
    const f = fixture();
    const source = f.catalog.entries[0]?.source;
    if (!source) throw new Error('Fixture source is missing.');
    const preview = {
      sessionId: f.catalog.result.context.sessionId,
      generation: 0,
      promptId: 'prompt:new' as const,
      sourceId: source.id,
      runOrder: 2,
      sequence: 1,
      token: 'frame:1',
      dimensions: null,
    };
    f.selection.reconcile(
      f.view,
      canvasOutputCatalog(
        { ...f.catalog.result, runOrder: 2, runStatus: 'running', preview },
        canvasSources(),
      ),
    );
    const state = f.views.read(f.view);
    for (let sequence = 2; sequence < 100; sequence++)
      f.selection.reconcile(
        f.view,
        canvasOutputCatalog(
          {
            ...f.catalog.result,
            runOrder: 2,
            runStatus: 'running',
            preview: { ...preview, sequence, token: `frame:${sequence}` },
          },
          canvasSources(),
        ),
      );
    expect(f.views.read(f.view)).toBe(state);
  });
  test('hands navigation over to the new preview while retiring previous finals from the catalog', () => {
    const f = fixture();
    const source = f.catalog.entries[0]?.source;
    if (!source) throw new Error('Fixture source is missing.');
    const preview = {
      sessionId: f.catalog.result.context.sessionId,
      generation: 0,
      promptId: 'prompt:new' as const,
      sourceId: source.id,
      runOrder: 2,
      sequence: 10,
      token: 'current',
      dimensions: null,
    };
    const catalog = canvasOutputCatalog(
      { ...f.catalog.result, runOrder: 2, runStatus: 'running', preview },
      canvasSources(),
    );
    f.selection.reconcile(f.view, catalog);
    expect(f.views.read(f.view).selection?.sourceId).toBe(source.id);
    expect(catalog.entries[0]?.summary).toMatchObject({ preview: true, previousRun: false });
    expect(f.selection.items(f.view, catalog).primary).toBeNull();
    expect(catalog.entries).toHaveLength(1);
    const completed = canvasCatalogFixture(2, 0, canvasSources().slice(0, 1));
    f.selection.reconcile(f.view, completed);
    expect(f.selection.items(f.view, completed).primary?.provenance.runOrder).toBe(2);
    expect(completed.entries[0]?.summary.preview).toBe(false);
  });
  test('uses stable authored order and qualifies duplicate labels without parsing markup', () => {
    const catalog = canvasCatalogFixture();
    expect(catalog.entries.map((entry) => entry.summary.label)).toEqual([
      '<b>Output</b>',
      'Nested / Preview (1)',
      'Nested / Preview (2)',
    ]);
    const filtered = canvasOutputCatalog(catalog.result, canvasSources().slice(0, 1));
    expect(filtered.entries).toHaveLength(1);
    expect(filtered.entries[0]?.source.executionPath).toBe('1');
  });

  test('scoped metadata cannot enroll candidates, change order, or cross generations', () => {
    const inventory = new CanvasSourceInventory();
    const context = canvasCatalogFixture().result.context;
    inventory.replace(context, canvasSources().slice(0, 1));
    inventory.refreshMetadata(
      context,
      canvasSources().map((source) => ({ ...source, label: 'Renamed', order: 99 })),
    );
    expect(inventory.read(context)).toHaveLength(1);
    expect(inventory.read(context)[0]).toMatchObject({ label: 'Renamed', order: 0 });
    expect(inventory.read({ ...context, generation: 1 })).toEqual([]);
    expect(() => inventory.replace(context, [canvasSources()[0]!, canvasSources()[0]!])).toThrow(
      'unique',
    );
    inventory.remove(context.sessionId);
    expect(inventory.read(context)).toEqual([]);
  });

  test('follows execution order rather than delayed old results and retains exact missing batches', () => {
    const f = fixture();
    expect(f.views.read(f.view).selection?.sourceId).toBe(canvasSources()[2]?.id);
    const next = canvasCatalogFixture(2);
    f.selection.reconcile(f.view, next);
    const delayed = { ...f.catalog, result: { ...f.catalog.result, runOrder: 2 } };
    f.selection.reconcile(f.view, delayed);
    expect(f.selection.items(f.view, next).primary?.provenance.runOrder).toBe(2);
    f.selection.apply(f.view, next, {
      type: 'select',
      selection: { sourceId: next.entries[2]!.source.id, batchIndex: 2 },
    });
    f.selection.reconcile(f.view, canvasCatalogFixture(2, 0, canvasSources(), 'canvas', 3));
    expect(f.views.read(f.view)).toMatchObject({ follow: 'manual', selection: { batchIndex: 2 } });
    expect(f.selection.items(f.view, next).primary).toBeNull();
    expect(canvasSelectedItem(next, f.views.read(f.view).selection)).toBeNull();
    f.selection.apply(f.view, next, { type: 'follow', mode: 'auto' });
    expect(f.selection.items(f.view, next).primary?.batchIndex).toBe(0);
  });

  test('retains two stage selections through final revisions, swaps, wipes, and surface movement', () => {
    const f = fixture();
    f.selection.apply(f.view, f.catalog, { type: 'comparison', enabled: true });
    const pinned = f.selection.items(f.view, f.catalog);
    expect(pinned.primary?.id).not.toBe(pinned.secondary?.id);
    const next = canvasCatalogFixture(1, 0, canvasSources(), 'canvas', 2);
    f.selection.reconcile(f.view, next);
    const refreshed = f.selection.items(f.view, next);
    expect(refreshed.primary?.provenance.sourceId).toBe(pinned.primary?.provenance.sourceId);
    expect(refreshed.secondary?.provenance.sourceId).toBe(pinned.secondary?.provenance.sourceId);
    expect(refreshed.primary?.id).not.toBe(pinned.primary?.id);
    f.selection.apply(f.view, next, { type: 'wipe', split: 0.8, orientation: 'horizontal' });
    const first = f.views.prepareAttachment(f.view, windowId('primary'));
    f.views.commitAttachment(first);
    const moved = f.views.prepareAttachment(f.view, windowId('another'));
    f.views.commitAttachment(moved);
    expect(f.selection.items(f.view, next)).toEqual(refreshed);
    f.selection.apply(f.view, next, { type: 'swap' });
    expect(f.selection.items(f.view, next)).toEqual({
      primary: refreshed.secondary,
      secondary: refreshed.primary,
    });
    expect(f.views.read(f.view).comparison).toMatchObject({
      split: 0.8,
      orientation: 'horizontal',
    });
  });

  test('keeps comparison disabled when only one actual final image is available', () => {
    const f = fixture();
    const single = canvasCatalogFixture(2, 0, canvasSources().slice(0, 1));
    f.selection.reconcile(f.view, single);
    f.selection.apply(f.view, single, { type: 'comparison', enabled: true });
    expect(f.views.read(f.view).comparison).toBeNull();
    expect(f.selection.items(f.view, single).secondary).toBeNull();
  });

  test('keeps the surviving comparison stage and marks its deleted partner unavailable', () => {
    const f = fixture();
    f.selection.apply(f.view, f.catalog, { type: 'comparison', enabled: true });
    const filtered = canvasOutputCatalog(f.catalog.result, canvasSources().slice(0, 1));
    f.selection.reconcile(f.view, filtered);
    expect(f.selection.items(f.view, filtered).primary).not.toBeNull();
    expect(f.selection.items(f.view, filtered).secondary).toBeNull();
    expect(f.views.read(f.view).selection?.sourceId).toBe(canvasSources()[0]?.id);
  });

  test('resets a replacement generation and releases pins without changing another view', () => {
    const f = fixture();
    const other = f.views.create(f.catalog.sessionId);
    f.selection.apply(f.view, f.catalog, { type: 'comparison', enabled: true });
    f.selection.apply(f.view, f.catalog, { type: 'link', enabled: false });
    f.selection.reconcile(f.view, canvasCatalogFixture(2, 1));
    expect(f.views.read(f.view)).toMatchObject({ comparison: null, follow: 'auto', linked: false });
    expect(f.views.read(other.viewId)).toEqual(other);
    expect(() =>
      f.selection.reconcile(f.view, canvasCatalogFixture(1, 0, canvasSources(), 'other')),
    ).toThrow('another workflow');
    f.selection.remove(f.view);
    f.selection.dispose();
  });
});

test('keeps the exact set when switching sources and rejects missing sets', () => {
  const f = fixture();
  const a = f.catalog.entries[0]!;
  const catalog = {
    ...f.catalog,
    entries: f.catalog.entries.map((entry) =>
      entry === a
        ? {
            ...entry,
            items: [
              ...entry.items,
              { ...entry.items[0]!, id: 'result:second' as const, batchIndex: 1 },
            ],
            summary: { ...entry.summary, batchCount: 2 },
          }
        : entry,
    ),
  };
  f.selection.apply(f.view, catalog, {
    type: 'select',
    selection: { sourceId: a.source.id, batchIndex: 1 },
  });
  f.selection.apply(f.view, catalog, {
    type: 'select-source',
    selection: { sourceId: catalog.entries[1]!.source.id, batchIndex: 1 },
  });
  expect(f.views.read(f.view).selection).toEqual({ sourceId: a.source.id, batchIndex: 1 });
  f.selection.apply(f.view, catalog, { type: 'batch', index: 8 });
  expect(f.views.read(f.view).selection).toEqual({ sourceId: a.source.id, batchIndex: 1 });
});

test('preserves grid navigation and last numbered set while switching source tabs', () => {
  const f = fixture();
  f.selection.apply(f.view, f.catalog, { type: 'overview', enabled: true });
  f.selection.apply(f.view, f.catalog, {
    type: 'select-source',
    selection: { sourceId: f.catalog.entries[0]!.source.id, batchIndex: 0 },
  });
  expect(f.views.read(f.view)).toMatchObject({
    mode: 'overview',
    selection: { sourceId: f.catalog.entries[0]!.source.id, batchIndex: 0 },
  });
  f.selection.apply(f.view, f.catalog, { type: 'batch', index: 0 });
  expect(f.views.read(f.view)).toMatchObject({
    mode: 'image',
    sourcePreview: false,
    selection: { batchIndex: 0 },
  });
});
