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

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import type { CanvasCommand } from '../domain/CanvasViewPort.js';
import type { CanvasSelection, CanvasViewState } from '../domain/CanvasViewState.js';
import { CanvasResultPins, type CanvasPinnedPair } from './CanvasResultPins.js';
import type { CanvasViewRegistry } from './CanvasViewRegistry.js';

type Frontier = readonly [number, number, number, number];
interface FollowCursor {
  generation: number;
  frontier: Frontier | null;
}
interface Candidate {
  selection: CanvasSelection;
  frontier: Frontier;
}

/** Resolve an exact slot without substituting a neighboring batch image. */
export function canvasSelectedItem(
  catalog: CanvasCatalogSnapshot,
  selection: CanvasSelection | null,
): WorkflowOutputItem | null {
  if (!selection) return null;
  return (
    catalog.entries
      .find((entry) => entry.source.id === selection.sourceId)
      ?.items.find((item) => item.batchIndex === selection.batchIndex) ?? null
  );
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
  readonly #pins = new CanvasResultPins();

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
      cursor = { generation: catalog.generation, frontier: null };
      this.#cursors.set(viewId, cursor);
      if (replacement) {
        this.#pins.clear(viewId);
        state = this.views.update(viewId, (current) => ({
          ...current,
          selection: null,
          comparison: null,
          follow: 'auto',
          mode: 'image',
        }));
      }
    }
    if (state.follow !== 'auto') return state;
    const candidate = autoCandidate(catalog);
    if (!candidate || (cursor.frontier && !later(candidate.frontier, cursor.frontier)))
      return state;
    // An older run can remain visible, but cannot displace the frontier after a new run begins.
    if (state.selection && candidate.frontier[0] < catalog.result.runOrder) return state;
    cursor.frontier = candidate.frontier;
    return this.views.update(viewId, (current) => ({ ...current, selection: candidate.selection }));
  }

  /** Apply navigation intents; rendering and resource commands belong to their own owners. */
  apply(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot, command: CanvasCommand): boolean {
    this.reconcile(viewId, catalog);
    const state = this.views.read(viewId);
    switch (command.type) {
      case 'select':
        this.#select(viewId, catalog, command.selection, false);
        return true;
      case 'batch':
        if (state.selection)
          this.#select(viewId, catalog, { ...state.selection, batchIndex: command.index }, false);
        return true;
      case 'follow':
        if (command.mode === 'auto') {
          this.#pins.clear(viewId);
          this.#cursors.set(viewId, { generation: catalog.generation, frontier: null });
          this.views.update(viewId, (current) => ({
            ...current,
            follow: 'auto',
            comparison: null,
            mode: 'image',
          }));
          this.reconcile(viewId, catalog);
        } else this.manual(viewId);
        return true;
      case 'overview':
        this.views.update(viewId, (current) => ({
          ...current,
          follow: 'manual',
          mode: command.enabled ? 'overview' : 'image',
        }));
        return true;
      case 'link':
        this.views.update(viewId, (current) => ({ ...current, linked: command.enabled }));
        return true;
      case 'comparison':
        this.#compare(viewId, catalog, command.enabled);
        return true;
      case 'select-comparison':
        if (state.comparison) this.#select(viewId, catalog, command.selection, true);
        return true;
      case 'swap': {
        const comparison = state.comparison;
        if (comparison?.secondary && state.selection) {
          const pair = this.#pins.read(viewId);
          if (pair) this.#pins.set(viewId, { primary: pair.secondary, secondary: pair.primary });
          this.views.update(viewId, (current) => ({
            ...current,
            follow: 'manual',
            selection: comparison.secondary,
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

  /** Suspend following as soon as a user starts an inspection gesture. */
  manual(viewId: CanvasViewId): void {
    this.views.update(viewId, (state) =>
      state.follow === 'manual' ? state : { ...state, follow: 'manual' },
    );
  }

  /** Read pinned revisions only while their current workflow source still exists. */
  items(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot): CanvasPinnedPair {
    const state = this.views.read(viewId);
    const pinned = state.comparison ? this.#pins.read(viewId) : null;
    const valid = (
      selection: CanvasSelection | null,
      item: WorkflowOutputItem | null,
    ): WorkflowOutputItem | null =>
      catalog.entries.some((entry) => entry.source.id === selection?.sourceId) ? item : null;
    return {
      primary: pinned
        ? valid(state.selection, pinned.primary)
        : canvasSelectedItem(catalog, state.selection),
      secondary: pinned ? valid(state.comparison?.secondary ?? null, pinned.secondary) : null,
    };
  }

  /** Release all selection-owned references when a view closes. */
  remove(viewId: CanvasViewId): void {
    this.#cursors.delete(viewId);
    this.#pins.clear(viewId);
  }

  /** End this integration without retaining lightweight comparison history. */
  dispose(): void {
    this.#cursors.clear();
    this.#pins.dispose();
  }

  /** Preserve exact requested slots, including explicit unavailable states. */
  #select(
    viewId: CanvasViewId,
    catalog: CanvasCatalogSnapshot,
    selection: CanvasSelection,
    secondary: boolean,
  ): void {
    if (!Number.isSafeInteger(selection.batchIndex) || selection.batchIndex < 0)
      throw new TypeError('Canvas batch index is invalid.');
    const state = this.views.read(viewId);
    if (state.comparison) {
      const pair = this.#pins.read(viewId) ?? { primary: null, secondary: null };
      this.#pins.set(
        viewId,
        secondary
          ? { ...pair, secondary: canvasSelectedItem(catalog, selection) }
          : { ...pair, primary: canvasSelectedItem(catalog, selection) },
      );
    }
    this.views.update(viewId, (current) => ({
      ...current,
      follow: 'manual',
      mode: 'image',
      selection: secondary ? current.selection : selection,
      comparison:
        secondary && current.comparison
          ? { ...current.comparison, secondary: selection }
          : current.comparison,
    }));
  }

  /** Pin only final still images, with an empty chooser when no distinct B is available. */
  #compare(viewId: CanvasViewId, catalog: CanvasCatalogSnapshot, enabled: boolean): void {
    const state = this.views.read(viewId);
    if (!enabled) {
      this.#pins.clear(viewId);
      this.views.update(viewId, (current) => ({ ...current, comparison: null, follow: 'manual' }));
      return;
    }
    if (state.comparison) return;
    const primary = canvasSelectedItem(catalog, state.selection);
    const secondary =
      catalog.entries
        .flatMap((entry) => entry.items)
        .find((item) => item.mediaKind === 'image' && item.id !== primary?.id) ?? null;
    this.#pins.set(viewId, { primary, secondary });
    this.views.update(viewId, (current) => ({
      ...current,
      follow: 'manual',
      mode: 'image',
      comparison: {
        secondary: secondary
          ? { sourceId: secondary.provenance.sourceId, batchIndex: secondary.batchIndex }
          : null,
        orientation: 'vertical',
        split: 0.5,
      },
    }));
  }
}
