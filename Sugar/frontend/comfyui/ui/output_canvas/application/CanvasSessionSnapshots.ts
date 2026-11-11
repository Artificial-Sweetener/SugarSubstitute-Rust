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
/** Project portable desktop views from the existing navigation and image-relative inspection owners. */
import type {
  SessionBinding,
  SessionEntry,
  SessionInspection,
  SessionOutput,
  SessionSelection,
  SessionView,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { WorkflowOutputSource } from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowOutputIdentities } from '../../execution/application/WorkflowOutputIdentities.js';
import type { WorkflowViewSnapshot } from '../../workflow/domain/WorkflowView.js';
import { outputSourceId } from '../../execution/domain/ExecutionIdentity.js';
import {
  canvasInspectionSlotKey,
  type CanvasInspection,
  type CanvasInspectionSlot,
} from '../domain/CanvasInspection.js';
import { canvasInspectionCatalog, canvasInspectionSlots } from './CanvasInspectionSlots.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import type { CanvasSessionCoordinator } from './CanvasSessionCoordinator.js';
import type { CanvasViewRegistry } from './CanvasViewRegistry.js';
import type { CanvasSelectionController } from './CanvasSelectionController.js';
import type { CanvasInspectionStore } from './CanvasInspectionStore.js';

/** Persist references and camera units without acquiring images, native surfaces or canvas DOM. */
export class CanvasSessionSnapshots {
  readonly #listeners = new Set<() => void>();
  readonly #views = new Map<CanvasViewId, () => void>();
  readonly #stops: (() => void)[] = [];

  constructor(
    private readonly coordinator: CanvasSessionCoordinator,
    private readonly views: CanvasViewRegistry,
    private readonly selection: CanvasSelectionController,
    private readonly inspections: CanvasInspectionStore,
  ) {}

  /** Observe lightweight intent before artifact receipts exist, excluding renderer resources and preview frames. */
  signature(context: WorkflowSessionContext): string {
    const view = this.coordinator.primary(context.sessionId);
    const catalog = view ? this.coordinator.catalog(view) : null;
    if (!view || !catalog) return '';
    this.inspections.synchronize(view, context.generation, canvasInspectionSlots(catalog));
    if (this.#listeners.size && !this.#views.has(view))
      this.#views.set(
        view,
        this.views.subscribe(view, () => this.#publish()),
      );
    const { revision: _revision, ...state } = this.views.read(view);
    return JSON.stringify([state, this.inspections.snapshot(view)]);
  }

  /** Observe actual per-view transactions and settled cameras; no rendering loop is involved. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    if (!this.#stops.length)
      this.#stops.push(
        this.coordinator.subscribe((view) => {
          if (view && !this.#views.has(view))
            this.#views.set(
              view,
              this.views.subscribe(view, () => this.#publish()),
            );
          this.#publish();
        }),
        this.coordinator.onRetired((view) => {
          this.#views.get(view)?.();
          this.#views.delete(view);
        }),
        this.inspections.subscribe(() => this.#publish()),
      );
    const active = this.coordinator.active();
    if (active && !this.#views.has(active))
      this.#views.set(
        active,
        this.views.subscribe(active, () => this.#publish()),
      );
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Capture navigation and inspection while preserving an inspected final's accepted source revision. */
  capture(
    context: WorkflowSessionContext,
    binding: SessionBinding,
    outputs: readonly SessionOutput[],
    identities: WorkflowOutputIdentities,
    preferences: WorkflowViewSnapshot,
    route: SessionView['route'],
  ): SessionView {
    const view = this.coordinator.primary(context.sessionId);
    const catalog = view ? this.coordinator.catalog(view) : null;
    if (!view || !catalog) throw new Error('Desktop canvas occurrence is unavailable.');
    const state = this.views.read(view);
    const sources = identities.sources(binding, catalog.sources);
    const all = [
      ...catalog.result.outputs,
      ...(catalog.result.scenes?.scenes.flatMap((scene) => scene.outputs) ?? []),
    ].flatMap((set) => set.items);
    const addresses = canvasInspectionCatalog(catalog);
    const outputFor = (slot: CanvasInspectionSlot | null, exact = false): SessionOutput | null => {
      if (!slot) return null;
      const visible =
        !exact &&
        catalog.entries
          .flatMap((entry) => entry.items)
          .find(
            (item) =>
              item.provenance.sourceId === slot.sourceId && item.batchIndex === slot.batchIndex,
          );
      const matches = visible
        ? [visible]
        : all.filter(
            (item) =>
              item.provenance.sourceId === slot.sourceId &&
              item.batchIndex === slot.batchIndex &&
              (!exact ||
                canvasInspectionSlotKey(addresses.get(item.id) ?? slot) ===
                  canvasInspectionSlotKey(slot)),
          );
      const item = matches.sort((a, b) => b.provenance.runOrder - a.provenance.runOrder)[0];
      return item
        ? (outputs.find((output) => output.id === identities.output(binding.entryId, item.id)) ??
            null)
        : null;
    };
    const select = (slot: CanvasSelection | null): SessionSelection | null => {
      const output = outputFor(slot);
      return output
        ? {
            outputId: output.id,
            sourceId: output.sourceId,
            sceneId: output.scene?.sceneId ?? null,
            batchIndex: output.batchIndex,
            listIndex: output.listIndex,
          }
        : null;
    };
    const slots = [...addresses.values()];
    this.inspections.synchronize(view, context.generation, slots);
    const cameras = this.inspections.snapshot(view);
    const source = state.selection
      ? sources.find((source) => outputSourceId(source.nodeId) === state.selection?.sourceId)
      : null;
    const selection = select(state.selection);
    const multiScene = (catalog.result.scenes?.scenes.length ?? 0) > 1;
    return Object.freeze({
      canvasEnabled: preferences.canvas,
      masonryEnabled: preferences.masonry,
      editorWidth: preferences.editorWidth,
      route,
      follow: state.follow,
      mode: state.mode,
      linked: state.linked,
      sourcePreview: state.sourcePreview,
      selection,
      comparison: Object.freeze({
        enabled: state.comparison !== null,
        selection: select(state.comparison?.secondary ?? null),
        orientation: state.comparison?.orientation ?? 'vertical',
        split: state.comparison?.split ?? 0.5,
      }),
      navigation: Object.freeze({
        sceneOverview: multiScene && state.scene === null && state.mode === 'overview',
        sceneId:
          state.scene && catalog.result.scenes
            ? identities.scene(binding.entryId, catalog.result.scenes.id, state.scene)
            : null,
        sourceId: selection?.sourceId ?? source?.id ?? null,
        batchIndex: state.selection?.batchIndex ?? 0,
        batchOverview: state.mode === 'overview' && !(multiScene && state.scene === null),
      }),
      sharedInspection: cameras.shared ? portableInspection(cameras.shared) : null,
      cameras: Object.freeze(
        cameras.individual.flatMap((sample) => {
          const output = outputFor(sample.selection, true);
          return output
            ? [
                Object.freeze({
                  outputId: output.id,
                  inspection: portableInspection(sample.inspection),
                }),
              ]
            : [];
        }),
      ),
    });
  }

  /** Restore only current occurrence references after the result owner has imported exact artifact IDs. */
  restore(
    context: WorkflowSessionContext,
    entry: SessionEntry,
    sources: readonly WorkflowOutputSource[],
  ): void {
    this.coordinator.restoreSources(context, sources);
    const view = this.coordinator.primary(context.sessionId);
    const catalog = view ? this.coordinator.catalog(view) : null;
    if (!view || !catalog) throw new Error('Restored canvas occurrence is unavailable.');
    const source = (id: string | null): CanvasSelection['sourceId'] | null => {
      const value = entry.sources.find((source) => source.id === id);
      return value ? outputSourceId(value.nodeId) : null;
    };
    const slot = (value: SessionSelection | null): CanvasSelection | null => {
      const id = value ? source(value.sourceId) : null;
      return id && value ? { sourceId: id, batchIndex: value.batchIndex } : null;
    };
    const navigation = entry.view.navigation;
    const navigationSource = source(navigation.sourceId);
    const selected =
      slot(entry.view.selection) ??
      (navigationSource ? { sourceId: navigationSource, batchIndex: navigation.batchIndex } : null);
    const scene =
      navigation.sceneId &&
      catalog.result.scenes?.scenes.some((scene) => scene.key === navigation.sceneId)
        ? navigation.sceneId
        : null;
    const items = [
      ...catalog.result.outputs,
      ...(catalog.result.scenes?.scenes.flatMap((scene) => scene.outputs) ?? []),
    ].flatMap((output) => output.items);
    const addresses = canvasInspectionCatalog(catalog);
    const slots = [...addresses.values()];
    const individual = new Map<
      string,
      { selection: CanvasInspectionSlot; inspection: CanvasInspection }
    >();
    for (const camera of entry.view.cameras) {
      const item = items.find((item) => item.id === camera.outputId);
      if (item) {
        const selection = addresses.get(item.id);
        if (!selection)
          throw new TypeError('Restored independent canvas inspection source is unavailable.');
        individual.set(canvasInspectionSlotKey(selection), {
          selection,
          inspection: runtimeInspection(camera.inspection),
        });
      }
    }
    this.inspections.restore(
      view,
      context.generation,
      {
        linked: entry.view.linked,
        shared: entry.view.sharedInspection ? runtimeInspection(entry.view.sharedInspection) : null,
        individual: [...individual.values()],
        presentedRun: catalog.result.presentedRunOrder,
        presentedGroup: catalog.result.scenes?.id ?? null,
      },
      slots,
    );
    this.selection.restore(view, catalog, {
      selection: selected,
      follow: entry.view.follow,
      mode: navigation.sceneOverview || navigation.batchOverview ? 'overview' : entry.view.mode,
      scene,
      linked: entry.view.linked,
      sourcePreview: entry.view.sourcePreview,
      comparison: entry.view.comparison.enabled
        ? {
            secondary: slot(entry.view.comparison.selection),
            orientation: entry.view.comparison.orientation,
            split: entry.view.comparison.split,
          }
        : null,
    });
  }

  /** Release observer lifetimes while leaving authoritative view owners to their composition. */
  dispose(): void {
    for (const stop of this.#stops.splice(0)) stop();
    for (const stop of this.#views.values()) stop();
    this.#views.clear();
    this.#listeners.clear();
  }
  #publish(): void {
    for (const listener of this.#listeners) listener();
  }
}

/** Keep relative centers unbounded and magnification measured per physical pixel. */
function portableInspection(value: CanvasInspection): SessionInspection {
  return Object.freeze({ mode: value.mode, ...value.region });
}
/** Restore exact units rather than treating spans as viewport fractions. */
function runtimeInspection({ mode, ...region }: SessionInspection): CanvasInspection {
  return Object.freeze({ mode, region: Object.freeze(region) });
}
