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
/** Exercise shared navigation against a surface that does not own browser image handles. */

import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import {
  CanvasSourceNavigation,
  type CanvasSourcePreparation,
  type CanvasSourcePreparationLease,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceNavigation.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import type { CanvasSelection } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewState.js';
import type { CanvasCommandScope } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';
import { canvasNavigationScope } from '../support/CanvasPortFixture.js';

interface PendingSurface {
  readonly selection: CanvasSelection;
  readonly scope: CanvasCommandScope;
  readonly resolve: (ready: boolean) => void;
  readonly reject: (error: Error) => void;
  ready: boolean;
  unavailable: boolean;
}

/** Hold the external surface at its readiness boundary without emulating selection policy. */
class StagedSurface implements CanvasSourcePreparation {
  pending: PendingSurface | null = null;
  commits = 0;

  /** Start a fresh surface-owned stage for the shared navigation request. */
  prepareSource(
    selection: CanvasSelection,
    _catalog: Parameters<CanvasSourcePreparation['prepareSource']>[1],
    _cells: Parameters<CanvasSourcePreparation['prepareSource']>[2],
    scope: CanvasCommandScope,
  ): CanvasSourcePreparationLease {
    this.cancelPreparation();
    let resolve = (_ready: boolean) => {};
    let reject = (_error: Error) => {};
    const ready = new Promise<boolean>((complete, fail) => {
      resolve = complete;
      reject = fail;
    });
    const pending: PendingSurface = {
      selection,
      scope,
      resolve,
      reject,
      ready: false,
      unavailable: false,
    };
    this.pending = pending;
    return {
      ready,
      get unavailable() {
        return pending.unavailable;
      },
      commit: (apply) => {
        if (this.pending !== pending || !pending.ready) return false;
        this.pending = null;
        this.commits++;
        return apply();
      },
    };
  }

  /** Resolve an interrupted stage without committing it. */
  cancelPreparation(): void {
    const pending = this.pending;
    this.pending = null;
    pending?.resolve(false);
  }

  /** Deliver a surface-owned readiness outcome for the currently staged source. */
  complete(ready: boolean, unavailable = false): void {
    if (!this.pending) throw new Error('No surface is staged.');
    this.pending.ready = ready;
    this.pending.unavailable = unavailable;
    this.pending.resolve(ready);
  }

  /** Expose an external surface failure to the shared handoff owner. */
  fail(error: Error): void {
    if (!this.pending) throw new Error('No surface is staged.');
    this.pending.reject(error);
  }
}

/** Compose the production catalog and selection owners with a replaceable surface boundary. */
function fixture() {
  let catalog = canvasCatalogFixture();
  const views = new CanvasViewRegistry(() => 'surface-navigation');
  const view = views.create(catalog.sessionId).viewId;
  const selection = new CanvasSelectionController(views, { latestOrder: () => 0 });
  selection.reconcile(view, catalog);
  const first = { sourceId: catalog.entries[0]!.source.id, batchIndex: 0 };
  const second = { sourceId: catalog.entries[1]!.source.id, batchIndex: 0 };
  const third = { sourceId: catalog.entries[2]!.source.id, batchIndex: 0 };
  selection.apply(view, catalog, { type: 'select', selection: first });
  const surface = new StagedSurface();
  const navigation = new CanvasSourceNavigation({
    resources: surface,
    selection,
    read: () => ({
      plan: canvasDisplayPlan(views.read(view), catalog, selection.items(view, catalog), null),
      catalog,
    }),
    cells: () => [],
  });
  return {
    navigation,
    surface,
    views,
    view,
    first,
    second,
    third,
    selectImmediately: (target: CanvasSelection) => {
      selection.apply(view, catalog, { type: 'select', selection: target });
    },
    replaceRun: () => {
      catalog = canvasCatalogFixture(2);
    },
  };
}

test('adopts an alternate surface and shared selection together after it is ready', async () => {
  const f = fixture();
  const scope = canvasNavigationScope();
  const change = f.navigation.select(f.second, scope);
  expect(f.surface.pending?.selection).toEqual(f.second);
  expect(f.surface.pending?.scope).toBe(scope);
  expect(f.views.read(f.view).selection).toEqual(f.first);
  f.surface.complete(true);
  expect(await change).toBe(true);
  expect(f.surface.commits).toBe(1);
  expect(f.views.read(f.view).selection).toEqual(f.second);
});

test('refuses a surface that becomes ready after its workflow run changes', async () => {
  const f = fixture();
  const change = f.navigation.select(f.second, canvasNavigationScope());
  f.replaceRun();
  f.surface.complete(true);
  expect(await change).toBe(false);
  expect(f.surface.commits).toBe(0);
  expect(f.views.read(f.view).selection).toEqual(f.first);
});

test('a superseding interaction cancels the pending handoff', async () => {
  const f = fixture();
  const change = f.navigation.select(f.second, canvasNavigationScope());
  f.navigation.cancel();
  expect(await change).toBe(false);
  expect(f.surface.commits).toBe(0);
  expect(f.views.read(f.view).selection).toEqual(f.first);
});

test('a source handoff cannot commit after its originating command loses authority', async () => {
  const f = fixture();
  const controller = new AbortController();
  const scope = {
    ...canvasNavigationScope(),
    signal: controller.signal,
    current: () => !controller.signal.aborted,
  };
  const change = f.navigation.select(f.second, scope);
  controller.abort();
  f.surface.complete(true);
  expect(await change).toBe(false);
  expect(f.surface.pending).toBeNull();
  expect(f.surface.commits).toBe(0);
  expect(f.views.read(f.view).selection).toEqual(f.first);
});

test('an obsolete handoff cannot cancel a newer source preparation', async () => {
  const f = fixture();
  const obsolete = f.navigation.select(f.second, canvasNavigationScope());
  f.selectImmediately(f.third);
  const current = f.navigation.select(f.first, canvasNavigationScope());
  expect(await obsolete).toBe(false);
  expect(f.surface.pending?.selection).toEqual(f.first);
  f.surface.complete(true);
  expect(await current).toBe(true);
  expect(f.surface.commits).toBe(1);
  expect(f.views.read(f.view).selection).toEqual(f.first);
});

test('retires a failed surface and preserves its cause without changing selection', async () => {
  const f = fixture();
  const change = f.navigation.select(f.second, canvasNavigationScope());
  const failure = new Error('Surface failed.');
  f.surface.fail(failure);
  await expect(change).rejects.toBe(failure);
  expect(f.surface.pending).toBeNull();
  expect(f.views.read(f.view).selection).toEqual(f.first);
});

test('an obsolete surface failure leaves the newer preparation intact', async () => {
  const f = fixture();
  const obsolete = f.navigation.select(f.second, canvasNavigationScope());
  const failure = new Error('Old surface failed.');
  f.surface.fail(failure);
  f.selectImmediately(f.third);
  const current = f.navigation.select(f.first, canvasNavigationScope());
  await expect(obsolete).rejects.toBe(failure);
  expect(f.surface.pending?.selection).toEqual(f.first);
  f.surface.complete(true);
  expect(await current).toBe(true);
  expect(f.views.read(f.view).selection).toEqual(f.first);
});

test('preserves both preparation and retirement failures', async () => {
  const f = fixture();
  const change = f.navigation.select(f.second, canvasNavigationScope());
  const failure = new Error('Surface failed.');
  const retirement = new Error('Surface retirement failed.');
  const cancel = f.surface.cancelPreparation.bind(f.surface);
  f.surface.cancelPreparation = () => {
    cancel();
    throw retirement;
  };
  f.surface.fail(failure);
  await expect(change).rejects.toMatchObject({ errors: [failure, retirement] });
  expect(f.views.read(f.view).selection).toEqual(f.first);
});
