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
/** The native view prepares browser media after authoritative semantics settle, never during widget painting. */
import { Facet, Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { featureEnabled } from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';
import { changesExactSource } from './exact-source.js';
import { featureProfile } from './features.js';
import { richRendering } from './presentation.js';
import { richProjection } from './rich-projection.js';
import { nativeInputSettlement } from './input-settlement.js';
import { loraPresentation, type LoraPresentation } from './lora-presentation.js';
import { loraMedia, installLoraMedia } from './lora-media-state.js';
import { LoraMediaPool } from './lora-media-pool.js';
import { decodeLoraMedia } from './lora-media-decoder.js';
import type { LoraMediaReference } from './lora-media-contracts.js';
import { completionWall } from './completion-wall-state.js';

/** One host boundary observes unexpected current preparation errors without logging names, image URLs or causes. */
export const loraMediaErrors = Facet.define<(cause: unknown) => void>();

/** Metadata owns descriptors, this view owns decoding/raster lifetime, and the state field owns only current prepared rendering input. */
class LoraMediaPreparation {
  private readonly view: EditorView;
  private readonly pool: LoraMediaPool;
  private queued = false;
  private disposed = false;
  private dirty = true;
  private input: {
    readonly snapshot: LoraPresentation['snapshot'];
    readonly sourceIdentity: symbol;
    readonly enabled: boolean;
    readonly wallReferences: readonly LoraMediaReference[] | null;
  } | null = null;
  /** Bind finite decoding and unexpected-error publication to the mounted native view. */
  constructor(view: EditorView) {
    this.view = view;
    this.pool = new LoraMediaPool(
      decodeLoraMedia,
      () => this.queue(),
      (cause) => {
        const reports = this.view.state.facet(loraMediaErrors);
        if (!reports.length) throw new Error('LoRA media preparation failed', { cause });
        for (const report of reports) report(cause);
      },
    );
    this.queue();
  }
  /** Synchronous source/context invalidation cancels decode without disposing pixels still used by the native projection. */
  update(update: ViewUpdate): void {
    const invalidated =
      update.transactions.some(changesExactSource) ||
      update.startState.field(loraPresentation).snapshot !==
        update.state.field(loraPresentation).snapshot ||
      update.startState.facet(richRendering) !== update.state.facet(richRendering) ||
      update.startState.facet(featureProfile) !== update.state.facet(featureProfile);
    if (invalidated) {
      this.input = null;
      this.pool.pause();
    }
    if (
      invalidated ||
      update.startState.field(completionWall)?.references !==
        update.state.field(completionWall)?.references ||
      update.startState.field(richProjection).sourceIdentity !==
        update.state.field(richProjection).sourceIdentity
    )
      this.queue();
  }
  /** Native preedit cancels publication and retains existing pixels until the projection leaves them. */
  compositionStart(): void {
    this.input = null;
    this.pool.pause();
    this.dirty = true;
  }
  /** Key/IME release resumes only retained dirty work; selection and ordinary key releases never traverse media records. */
  resume(): void {
    if (this.dirty) this.queue();
  }
  /** Teardown retires all queued publication, decode callbacks and retained native pixels. */
  destroy(): void {
    this.disposed = true;
    this.input = null;
    this.pool.dispose();
  }
  /** Coalesce background work beyond the native transaction, input and paint stack. */
  private queue(): void {
    this.dirty = true;
    if (this.disposed || this.queued) return;
    this.queued = true;
    this.view.requestMeasure({
      key: this,
      /** Native measurement flushes accepted browser input before asynchronous raster publication. */
      read: () => !this.disposed,
      /** Decoder selection and state publication stay outside the measurement/paint stack. */
      write: (live) =>
        queueMicrotask(() => {
          this.queued = false;
          if (live && !this.disposed) this.prepare();
        }),
    });
  }
  /** Consume cached metadata only after native key/IME settlement and a source-current projection. */
  private prepare(): void {
    const state = this.view.state;
    if (this.view.compositionStarted || state.facet(nativeInputSettlement).hasHeldKey(this.view))
      return;
    const enabled =
      state.facet(richRendering) && featureEnabled(state.facet(featureProfile), 'lora_syntax');
    const presentation = state.field(loraPresentation),
      sourceIdentity = state.field(editorFreshness).source;
    const wallReferences = state.field(completionWall)?.references ?? null;
    if (state.field(richProjection).sourceIdentity !== sourceIdentity) return;
    if (enabled && presentation.sourceIdentity !== sourceIdentity) return;
    if (
      !this.input ||
      this.input.snapshot !== presentation.snapshot ||
      this.input.sourceIdentity !== sourceIdentity ||
      this.input.enabled !== enabled ||
      this.input.wallReferences !== wallReferences
    ) {
      this.input = {
        snapshot: presentation.snapshot,
        sourceIdentity,
        enabled,
        wallReferences,
      };
      const references: LoraMediaReference[] = [];
      if (enabled && presentation.snapshot.status === 'ready')
        for (const resource of presentation.snapshot.resources)
          if (resource.resolution === 'found' && resource.thumbnail.status === 'ready')
            references.push(resource.thumbnail);
      if (wallReferences) references.push(...wallReferences);
      this.pool.select(references);
    }
    const media = this.pool.snapshot();
    const currentMedia = state.field(loraMedia);
    this.dirty = false;
    if (
      media.size === currentMedia.size &&
      [...media].every(([reference, value]) => currentMedia.get(reference) === value)
    )
      return;
    this.view.dispatch({
      effects: installLoraMedia.of({
        freshness: state.field(editorFreshness),
        media,
      }),
      annotations: Transaction.addToHistory.of(false),
    });
  }
}

/** The native plugin shares the editor's composition and physical-key owners rather than creating another input scheduler. */
export const loraMediaPreparation = ViewPlugin.fromClass(LoraMediaPreparation, {
  eventObservers: {
    /** Keep unfinished native text outside media preparation. */
    compositionstart() {
      this.compositionStart();
    },
    /** The existing semantic refresh publishes after native composition settlement. */
    compositionend() {
      this.resume();
    },
    /** An unchanged key cycle can release a metadata update that arrived while typing was held. */
    keyup() {
      this.resume();
    },
    /** Focus transfer releases physical key ownership through the native input owner. */
    blur() {
      this.resume();
    },
  },
});
