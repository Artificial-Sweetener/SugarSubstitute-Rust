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
/** Translate visible workflow descriptors into bounded browser image admissions. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { WorkflowPreview } from '../../execution/domain/WorkflowPreview.js';
import type { CanvasImageInput } from '../domain/CanvasImageResource.js';
import {
  canvasPreviewLane,
  type CanvasDisplayPlan,
  type CanvasDisplaySlot,
} from '../application/CanvasDisplayPlan.js';
import type {
  CanvasImageResources,
  CanvasVisibleImage,
} from '../application/CanvasImageResources.js';
import type { CanvasImageBudget } from '../application/CanvasImageBudget.js';
import { CANVAS_IMAGE_BYTE_LIMIT } from '../application/CanvasImageBudget.js';
import type {
  CanvasImageAllowances,
  CanvasImageAllowanceInput,
} from '../application/CanvasImageAllowances.js';
import type { BrowserCanvasImagePayload } from './BrowserCanvasImageLoader.js';
import type { CanvasOverviewCell } from '../application/CanvasOverviewWindow.js';
import {
  CanvasImagePreparation,
  type CanvasImagePreparationLease,
} from '../application/CanvasImagePreparation.js';
import { canvasSelectedItem } from '../application/CanvasContentSelection.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';

/** Keep ephemeral byte delivery separate from catalog updates and final-file loading. */
export class BrowserCanvasResources {
  readonly #preparation: CanvasImagePreparation<BrowserCanvasImagePayload, HTMLImageElement>;
  #plan: CanvasDisplayPlan | null = null;
  #budgetScope = '';
  #previewStarted = false;
  #overviewImages: CanvasVisibleImage<BrowserCanvasImagePayload>[] = [];
  #overviewPreviewSlot: string | null = null;
  #disposed = false;
  #active = true;
  #refreshNeeded = false;

  /** Use existing authoritative resource/approval owners and a host-qualified final URL adapter. */
  constructor(
    readonly resources: CanvasImageResources<BrowserCanvasImagePayload, HTMLImageElement>,
    private readonly budget: CanvasImageBudget,
    private readonly allowances: CanvasImageAllowances,
    private readonly url: (item: WorkflowOutputItem) => string,
  ) {
    this.#preparation = new CanvasImagePreparation(resources);
  }

  /** Warm only the requested source's visible finals under the current shared budget. */
  prepareSource(
    selection: CanvasSelection,
    catalog: CanvasCatalogSnapshot,
    cells: readonly CanvasOverviewCell[] | null,
  ): CanvasImagePreparationLease | null {
    const plan = this.#plan;
    if (!plan || cells?.some((cell) => cell.preview)) return null;
    // Poster generation must never hold a mixed-media source-tab handoff behind a model load.
    if (cells?.some((cell) => cell.item && cell.item.mediaKind !== 'image')) return null;
    const targets = cells ?? [
      { key: 'primary', selection, item: canvasSelectedItem(catalog, selection) },
    ];
    const images: CanvasVisibleImage<BrowserCanvasImagePayload>[] = [];
    for (const target of targets) {
      const input = this.#final(
        {
          selection: target.selection,
          item: target.item,
          sourcePresent: plan.availableSources.includes(target.selection.sourceId),
        },
        cells !== null,
      );
      if (!input) return null;
      images.push({ slot: target.key, input: cells ? { ...input, role: 'thumbnail' } : input });
    }
    return images.length ? this.#preparation.prepare(images) : null;
  }

  /** Relinquish pending source navigation when another intent supersedes it. */
  cancelPreparation(): void {
    this.#preparation.cancel();
  }

  /** Report staging independently of the currently displayed frame's ready status. */
  get preparing(): boolean {
    return this.#preparation.pending;
  }

  /** Replace resource interests only at a logical display change, preserving same-run pending previews. */
  reconcile(plan: CanvasDisplayPlan, retry = false): void {
    if (this.#disposed) return;
    if (
      this.#plan &&
      (this.#plan.view.viewId !== plan.view.viewId ||
        this.#plan.view.sessionId !== plan.view.sessionId)
    )
      throw new TypeError('Browser image resources belong to another view.');
    const replaced = this.#plan !== null && this.#plan.generation !== plan.generation;
    const changed = this.#refreshNeeded || this.#plan?.signature !== plan.signature;
    this.#plan = plan;
    if (!this.#active) return;
    this.#refreshNeeded = false;
    if (replaced) this.#preparation.update([]);
    this.#configureBudget();
    this.resources.retirePreviewsExcept(plan.preview ? canvasPreviewLane(plan.preview) : null);
    if (!changed && !retry) return;
    this.#previewStarted = false;
    if (plan.view.mode === 'overview') return;
    this.#overviewImages = [];
    this.#overviewPreviewSlot = null;
    const images: CanvasVisibleImage<BrowserCanvasImagePayload>[] = [];
    if (plan.view.mode === 'image') {
      for (const [name, slot] of [
        ['primary', plan.primary],
        ['secondary', plan.secondary],
      ] as const) {
        const input = slot ? this.#final(slot) : null;
        if (input) images.push({ slot: name, input });
      }
    }
    // Deleted sources relinquish their displayed lane before ordinary replacement can retain it.
    for (const snapshot of this.resources.read())
      if (snapshot.frame && !plan.availableSources.includes(snapshot.frame.selection.sourceId))
        this.resources.releaseSlot(snapshot.slot);
    this.#preparation.update(
      images,
      retry,
      plan.preview && plan.primary
        ? {
            slot: 'primary',
            lane: canvasPreviewLane(plan.preview),
            selection: plan.primary.selection,
            role: 'main',
          }
        : null,
    );
    this.#previewStarted = Boolean(
      plan.preview && this.resources.hasPreviewInterest(canvasPreviewLane(plan.preview)),
    );
  }

  /** Admit only the overview's intersecting final-image cells through the same byte/decode limits. */
  overview(cells: readonly CanvasOverviewCell[]): void {
    const plan = this.#plan;
    if (!plan || plan.view.mode !== 'overview' || !this.#active || this.#disposed) return;
    const images: CanvasVisibleImage<BrowserCanvasImagePayload>[] = [];
    this.#overviewPreviewSlot = null;
    for (const cell of cells) {
      if (cell.preview) this.#overviewPreviewSlot = cell.key;
      const input = this.#final(
        {
          selection: cell.selection,
          item: cell.item,
          sourcePresent: plan.availableSources.includes(cell.selection.sourceId),
        },
        true,
      );
      if (input) images.push({ slot: cell.key, input: { ...input, role: 'thumbnail' } });
    }
    this.#overviewImages = images;
    const previewCell = cells.find((cell) => cell.preview);
    this.#preparation.update(
      images,
      false,
      plan.preview && previewCell
        ? {
            slot: previewCell.key,
            lane: canvasPreviewLane(plan.preview),
            selection: previewCell.selection,
            role: 'thumbnail',
          }
        : null,
    );
    this.#previewStarted = Boolean(
      plan.preview && this.resources.hasPreviewInterest(canvasPreviewLane(plan.preview)),
    );
  }

  /** Refuse stale/foreign bytes before handing their ownership to a visible resource slot. */
  receive(preview: WorkflowPreview, blob: Blob): void {
    const plan = this.#plan;
    const current = plan?.preview;
    const slot = plan?.primary;
    if (
      this.#disposed ||
      !this.#active ||
      !plan ||
      !current ||
      !slot ||
      preview.token !== current.token ||
      canvasPreviewLane(preview) !== canvasPreviewLane(current)
    )
      return;
    const lane = canvasPreviewLane(preview);
    const input: CanvasImageInput<BrowserCanvasImagePayload> = {
      key: preview.token,
      runOrder: preview.runOrder,
      selection: { sourceId: preview.sourceId, batchIndex: 0 },
      lane,
      fallbackKey: slot.item?.id ?? null,
      dimensions:
        this.resources.dimensions(lane) ??
        preview.dimensions ??
        this.allowances.dimensions(plan.view.viewId, plan.generation, lane),
      role: plan.view.mode === 'overview' ? 'thumbnail' : 'main',
      preview: true,
      payload: { kind: 'preview', blob },
    };
    this.#previewStarted = true;
    const target = plan.view.mode === 'overview' ? this.#overviewPreviewSlot : 'primary';
    if (!target) return;
    if (!this.resources.offer(target, input))
      this.#preparation.update([
        ...this.#overviewImages.filter((image) => image.slot !== target),
        { slot: target, input },
      ]);
  }

  /** Approve exactly the current logical image/pair and resume privately queued work. */
  admit(): void {
    const plan = this.#plan;
    if (!plan || this.#disposed) return;
    const images = this.#approvalImages();
    if (!images.length) return;
    this.allowances.grant(plan.view.viewId, plan.generation, images);
    this.#configureBudget();
    this.resources.admissionChanged();
    if (!plan.preview) this.reconcile(plan, true);
  }

  /** Remember measured approved geometry without resetting another unknown partner's admission epoch. */
  measured(): void {
    const plan = this.#plan;
    if (!plan) return;
    for (const snapshot of this.resources.read()) {
      const frame = snapshot.frame;
      if (!frame) continue;
      const key = frame.preview && plan.preview ? canvasPreviewLane(plan.preview) : frame.key;
      this.allowances.measure(plan.view.viewId, plan.generation, key, frame.dimensions);
    }
  }

  /** Release hidden allocation pixels and rebuild current final interests on visible reactivation. */
  active(active: boolean): void {
    if (this.#disposed || this.#active === active) return;
    this.#active = active;
    this.#refreshNeeded = true;
    if (!active) this.#preparation.cancel();
    this.resources.active(active);
    if (active && this.#plan) this.reconcile(this.#plan);
  }

  /** Report whether a current preview has received bytes, independently of final fallback visibility. */
  get previewStarted(): boolean {
    return this.#previewStarted;
  }

  /** Release this attachment's images and bytes while keeping view-scoped lightweight approval. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#plan = null;
    this.#preparation.dispose();
    this.resources.dispose();
  }

  /** Build a full-fidelity request from an exact selected final. */
  #final(
    slot: CanvasDisplaySlot,
    thumbnail = false,
  ): CanvasImageInput<BrowserCanvasImagePayload> | null {
    const plan = this.#plan;
    const item = slot.item;
    if (
      !plan ||
      !slot.sourcePresent ||
      !item ||
      item.mediaKind === 'unsupported' ||
      (!thumbnail && item.mediaKind !== 'image')
    )
      return null;
    const poster = item.mediaKind !== 'image';
    return {
      key: item.id,
      runOrder: item.provenance.runOrder,
      selection: slot.selection,
      lane: item.id,
      fallbackKey: null,
      dimensions:
        this.resources.dimensions(item.id) ??
        (poster ? { width: 512, height: 512 } : item.dimensions) ??
        this.allowances.dimensions(plan.view.viewId, plan.generation, item.id),
      role: 'main',
      preview: false,
      payload: poster
        ? { kind: 'media', item, url: this.url(item) }
        : { kind: 'file', url: this.url(item) },
    };
  }

  /** Preserve multiplicity for an explicitly selected duplicate A/B image. */
  #approvalImages(): readonly CanvasImageAllowanceInput[] {
    const plan = this.#plan;
    if (!plan || plan.view.mode !== 'image') return [];
    const images: CanvasImageAllowanceInput[] = [];
    if (plan.preview) {
      const key = canvasPreviewLane(plan.preview);
      images.push({ key, dimensions: this.resources.dimensions(key) ?? plan.preview.dimensions });
    } else if (plan.primary?.item?.mediaKind === 'image') {
      const item = plan.primary.item;
      images.push({
        key: item.id,
        dimensions: this.resources.dimensions(item.id) ?? item.dimensions,
      });
    }
    if (plan.secondary?.item?.mediaKind === 'image') {
      const item = plan.secondary.item;
      images.push({
        key: item.id,
        dimensions: this.resources.dimensions(item.id) ?? item.dimensions,
      });
    }
    return images;
  }

  /** Configure one budget epoch per logical set/approval, never per successfully measured frame. */
  #configureBudget(): void {
    const plan = this.#plan;
    if (!plan) return;
    const keys = this.#approvalImages().map((image) => image.key);
    const allowance = this.allowances.read(plan.view.viewId, plan.generation, keys);
    const scope = JSON.stringify([plan.generation, keys.sort(), allowance?.approvalId ?? null]);
    if (scope === this.#budgetScope) return;
    this.#budgetScope = scope;
    this.budget.allowance(
      allowance?.bytes ?? CANVAS_IMAGE_BYTE_LIMIT,
      allowance?.unknownLoads ?? 0,
    );
  }
}
