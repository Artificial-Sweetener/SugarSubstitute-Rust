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
/** Own navigation, execution following, and bounded comparison revision intent. */

import type { OutputSourceId } from '../../execution/domain/ExecutionIdentity.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import type { CanvasCommand } from '../domain/CanvasViewPort.js';
import type { CanvasSelection, CanvasViewState } from '../domain/CanvasViewState.js';
import type { CanvasViewRegistry } from './CanvasViewRegistry.js';
import { canvasComparisonPair, type CanvasImagePair } from './CanvasComparisonSelection.js';
import {
  canvasSelectedItem,
  canvasAvailableSelection,
  canvasItemSelection,
  canvasBatchGridCatalog,
} from './CanvasContentSelection.js';

type Frontier = readonly [number, number, number, number];
interface FollowCursor {
  generation: number;
  frontier: Frontier | null;
  presentedRun: number;
}
interface Candidate {
  selection: CanvasSelection;
  frontier: Frontier;
}

/** Compare attributable execution progress independently of delivery order. */
function later(left: Frontier, right: Frontier): boolean {
  for (let index = 0; index < left.length; index++) {
    const a = left[index] ?? 0;
    const b = right[index] ?? 0;
    if (a !== b) return a > b;
  }
  return false;
}

/** Find the newest authored execution frontier, including a source-level live preview. */
function autoCandidate(catalog: CanvasCatalogSnapshot): Candidate | null {
  let candidate: Candidate | null = null;
  for (const entry of catalog.entries) {
    for (const item of entry.items) {
      const frontier: Frontier = [
        item.provenance.runOrder,
        entry.source.order,
        1,
        item.provenance.revision,
      ];
      if (!candidate || later(frontier, candidate.frontier))
        candidate = {
          selection: { sourceId: entry.source.id, batchIndex: item.batchIndex },
          frontier,
        };
    }
    if (entry.preview) {
      const frontier: Frontier = [
        entry.preview.runOrder,
        entry.source.order,
        0,
        entry.preview.sequence,
      ];
      if (!candidate || later(frontier, candidate.frontier))
        candidate = { selection: { sourceId: entry.source.id, batchIndex: 0 }, frontier };
    }
  }
  return candidate;
}

/** Keep per-view selection policy separate from immutable registry storage and result ingestion. */
export class CanvasSelectionController {
  readonly #cursors = new Map<CanvasViewId, FollowCursor>();

  /** Use the existing view authority for every published state transition. */
  constructor(private readonly views: CanvasViewRegistry) {}

  /** Reconcile a named view without making background deliveries select another workflow. */
  reconcile(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot): CanvasViewState {
    let state = this.views.read(viewId);
    if (state.sessionId !== catalog.sessionId)
      throw new TypeError('Canvas catalog belongs to another workflow.');
    let cursor = this.#cursors.get(viewId);
    if (!cursor || cursor.generation !== catalog.generation) {
      const replacement = Boolean(cursor);
      cursor = { generation: catalog.generation, frontier: null, presentedRun: 0 };
      this.#cursors.set(viewId, cursor);
      if (replacement) {
        state = this.views.update(viewId, (current) => ({
          ...current,
          selection: null,
          sourcePreview: true,
          comparison: null,
          follow: 'auto',
          mode: 'image',
        }));
      }
    }
    const candidate = autoCandidate(catalog);
    if (candidate && candidate.frontier[0] > cursor.presentedRun) {
      cursor.presentedRun = candidate.frontier[0];
      cursor.frontier = null;
      state = this.views.update(viewId, (current) => ({
        ...current,
        follow: 'auto',
        sourcePreview: true,
        comparison: null,
        mode: 'image',
      }));
    }
    if (state.selection && state.sourcePreview) {
      const activeCatalog = catalog;
      const entry = activeCatalog.entries.find(
        (entry) => entry.source.id === state.selection?.sourceId,
      );
      if (!entry?.preview) {
        const available = canvasAvailableSelection(activeCatalog, state.selection);
        if (available.batchIndex !== state.selection.batchIndex)
          state = this.views.update(viewId, (current) => ({ ...current, selection: available }));
      }
    }
    if (state.follow !== 'auto') return state;
    const currentSourceExists = catalog.entries.some(
      (entry) => entry.source.id === state.selection?.sourceId,
    );
    if (
      !candidate ||
      (currentSourceExists && cursor.frontier && !later(candidate.frontier, cursor.frontier))
    )
      return state;
    // An older run can remain visible, but cannot displace the frontier after a new run begins.
    if (state.selection && candidate.frontier[0] < catalog.result.runOrder) return state;
    cursor.frontier = candidate.frontier;
    const batch = canvasBatchGridCatalog(catalog, candidate.selection, true);
    const mode = (batch.entries[0]?.summary.batchCount ?? 0) > 1 ? 'overview' : 'image';
    if (
      state.selection?.sourceId === candidate.selection.sourceId &&
      state.selection.batchIndex === candidate.selection.batchIndex &&
      state.mode === mode
    )
      return state;
    return this.views.update(viewId, (current) => ({
      ...current,
      selection: candidate.selection,
      mode,
      sourcePreview: true,
    }));
  }

  /** Apply navigation intents; rendering and resource commands belong to their own owners. */
  apply(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot, command: CanvasCommand): boolean {
    this.reconcile(viewId, catalog);
    const state = this.views.read(viewId);
    switch (command.type) {
      case 'select':
        this.#select(viewId, catalog, command.selection, false, false);
        return true;
      case 'select-source':
        this.#select(viewId, catalog, command.selection, false, true);
        return true;
      case 'batch':
        if (
          state.selection &&
          canvasSelectedItem(catalog, { ...state.selection, batchIndex: command.index })
        )
          this.#select(
            viewId,
            catalog,
            { ...state.selection, batchIndex: command.index },
            false,
            false,
          );
        return true;
      case 'follow':
        if (command.mode === 'auto') {
          this.#cursors.set(viewId, {
            generation: catalog.generation,
            frontier: null,
            presentedRun: this.#cursors.get(viewId)?.presentedRun ?? 0,
          });
          this.views.update(viewId, (current) => ({
            ...current,
            follow: 'auto',
            sourcePreview: true,
            comparison: null,
            mode: 'image',
          }));
          this.reconcile(viewId, catalog);
        } else this.manual(viewId);
        return true;
      case 'overview':
        if (
          command.enabled &&
          (!state.selection ||
            !catalog.entries.some(
              (entry) => entry.source.id === state.selection?.sourceId && entry.items.length,
            ))
        )
          return true;
        this.views.update(viewId, (current) => ({
          ...current,
          follow: 'manual',
          mode: command.enabled ? 'overview' : 'image',
          comparison: null,
        }));
        return true;
      case 'link':
        this.views.update(viewId, (current) => ({ ...current, linked: command.enabled }));
        return true;
      case 'comparison':
        this.#compare(viewId, catalog, command.enabled);
        return true;
      case 'select-comparison':
        if (state.comparison) this.#select(viewId, catalog, command.selection, true, false);
        return true;
      case 'swap': {
        const comparison = state.comparison;
        if (comparison?.secondary && state.selection) {
          this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            selection: comparison.secondary,
            sourcePreview: false,
            comparison: { ...comparison, secondary: state.selection },
          }));
        }
        return true;
      }
      case 'wipe': {
        const comparison = state.comparison;
        if (comparison)
          this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            comparison: {
              ...comparison,
              split: command.split,
              orientation: command.orientation,
            },
          }));
        return true;
      }
      default:
        return false;
    }
  }

  /** Hold deliberate content navigation for the current output session. */
  manual(viewId: CanvasViewId): void {
    this.views.update(viewId, (state) =>
      state.follow === 'manual' ? state : { ...state, follow: 'manual' },
    );
  }

  /** Resolve both semantic selections to current finals without retaining superseded revisions. */
  items(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot): CanvasImagePair {
    const state = this.views.read(viewId);
    return {
      primary: canvasSelectedItem(catalog, state.selection),
      secondary: canvasSelectedItem(catalog, state.comparison?.secondary ?? null),
    };
  }

  /** Admit source-level preview interest without assigning unknown preview bytes to an explicit batch. */
  allowsSourcePreview(
    viewId: CanvasViewId,
    catalog: CanvasCatalogSnapshot,
    source: OutputSourceId,
  ): boolean {
    const state = this.views.read(viewId);
    return (
      state.sessionId === catalog.sessionId &&
      this.#cursors.get(viewId)?.generation === catalog.generation &&
      (state.mode === 'image' ||
        (state.mode === 'overview' &&
          (catalog.result.runStatus === 'running' || catalog.result.runStatus === 'queued'))) &&
      state.comparison === null &&
      state.sourcePreview &&
      state.selection?.sourceId === source &&
      catalog.entries.some((entry) => entry.source.id === source)
    );
  }

  /** Read qualified transient metadata for the selected visible source without retaining its bytes. */
  preview(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot): WorkflowPreview | null {
    const preview = catalog.result.preview;
    return preview && this.allowsSourcePreview(viewId, catalog, preview.sourceId) ? preview : null;
  }

  /** Release all selection-owned references when a view closes. */
  remove(viewId: CanvasViewId): void {
    this.#cursors.delete(viewId);
  }

  /** Release navigation state at integration teardown. */
  dispose(): void {
    this.#cursors.clear();
  }

  /** Preserve exact requested slots, including explicit unavailable states. */
  #select(
    viewId: CanvasViewId,
    catalog: CanvasCatalogSnapshot,
    selection: CanvasSelection,
    secondary: boolean,
    sourcePreview: boolean,
  ): void {
    if (!Number.isSafeInteger(selection.batchIndex) || selection.batchIndex < 0)
      throw new TypeError('Canvas batch index is invalid.');
    const state = this.views.read(viewId);
    if (sourcePreview || state.comparison) {
      const entry = catalog.entries.find((entry) => entry.source.id === selection.sourceId);
      if (!entry) return;
      if (sourcePreview && state.mode === 'overview') {
        if (!entry.items.length && !entry.preview) return;
      } else if (!canvasSelectedItem(catalog, selection)) {
        if (!sourcePreview || !entry.preview || entry.items.length) return;
      }
    }
    this.views.update(viewId, (current) => ({
      ...current,
      follow: 'manual',
      mode: sourcePreview && current.mode === 'overview' ? 'overview' : 'image',
      selection: secondary ? current.selection : selection,
      sourcePreview: secondary ? current.sourcePreview : sourcePreview,
      comparison:
        secondary && current.comparison
          ? { ...current.comparison, secondary: selection }
          : current.comparison,
    }));
  }

  /** Start reveal comparison with the first and last final stages in the selected batch. */
  #compare(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot, enabled: boolean): void {
    const state = this.views.read(viewId);
    if (!enabled) {
      this.views.update(viewId, (current) => ({ ...current, comparison: null, follow: 'manual' }));
      return;
    }
    if (state.comparison) return;
    const pair = canvasComparisonPair(catalog, state.selection?.batchIndex ?? 0);
    if (!pair?.primary || !pair.secondary) return;
    const { primary, secondary } = pair;
    this.views.update(viewId, (current) => ({
      ...current,
      follow: 'manual',
      mode: 'image',
      selection: canvasItemSelection(primary),
      sourcePreview: false,
      comparison: {
        secondary: canvasItemSelection(secondary),
        orientation: 'vertical',
        split: 0.5,
      },
    }));
  }
}
