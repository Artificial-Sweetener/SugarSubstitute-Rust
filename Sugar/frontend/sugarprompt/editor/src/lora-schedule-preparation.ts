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
/** One finite native lifetime prepares effective schedules from existing semantics and resources outside input and paint. */
import { Transaction } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { featureEnabled, SceneLoraSession } from '../../services/src/index.js';
import { featureProfile } from './features.js';
import { editorFreshness } from './freshness.js';
import { nativeInputSettlement } from './input-settlement.js';
import { semanticStructures } from './structure.js';
import { loraPresentation } from './lora-presentation.js';
import { prepareLoraTriggerEligibility } from './lora-trigger-eligibility.js';
import { installLoraSchedule, loraSchedulingHost, nativeLoraSchedule } from './lora-schedule.js';

/** Coalesce dependency publications; the shared service memo owns the actual model/index reuse. */
class LoraSchedulePreparation {
  private readonly view: EditorView;
  private scopes: SceneLoraSession | null = null;
  private queued = false;
  private disposed = false;
  private composing = false;
  /** Mounted baselines qualify through the same semantic settlement as later edits. */
  constructor(view: EditorView) {
    this.view = view;
    this.queue();
  }
  /** Any prepared dependency or key release can finish the pending source. */
  update(_update: ViewUpdate): void {
    this.queue();
  }
  /** No queued publication or captured metadata survives the native lifetime. */
  destroy(): void {
    this.disposed = true;
    this.scopes?.dispose();
    this.scopes = null;
  }
  /** Preedit cannot prepare or publish configured words. */
  compositionStart(): void {
    this.composing = true;
  }
  /** Committed DOM settlement precedes semantic readiness and scheduling. */
  compositionEnd(): void {
    this.composing = false;
    queueMicrotask(() => queueMicrotask(() => this.queue()));
  }
  /** Wait for cached source semantics and source-current resource publication, without invoking a parser. */
  queue(): void {
    const state = this.view.state;
    if (!featureEnabled(state.facet(featureProfile), 'lora_trigger_words')) {
      this.scopes?.dispose();
      this.scopes = null;
      return;
    }
    if (
      this.disposed ||
      this.queued ||
      this.composing ||
      this.view.compositionStarted ||
      this.view.composing ||
      state.facet(nativeInputSettlement).hasHeldKey(this.view) ||
      state.field(nativeLoraSchedule) ||
      !state.field(semanticStructures).native ||
      state.field(loraPresentation).sourceIdentity !== state.field(editorFreshness).source
    )
      return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      if (this.disposed || this.composing || this.view.compositionStarted || this.view.composing)
        return;
      const current = this.view.state,
        semantic = current.field(semanticStructures).native,
        resources = current.field(loraPresentation),
        freshness = current.field(editorFreshness),
        host = current.facet(loraSchedulingHost);
      if (
        !semantic ||
        !featureEnabled(current.facet(featureProfile), 'lora_trigger_words') ||
        current.field(nativeLoraSchedule) ||
        current.facet(nativeInputSettlement).hasHeldKey(this.view) ||
        resources.sourceIdentity !== freshness.source
      )
        return;
      this.scopes ??= new SceneLoraSession();
      const scopes = this.scopes.prepareDocument(
        semantic.loraScopes,
        resources.snapshot,
        host.catalogRevision,
        host.context,
      );
      this.view.dispatch({
        effects: installLoraSchedule.of(
          Object.freeze({
            freshness,
            host,
            resources,
            scopes,
            hasTriggersAt: prepareLoraTriggerEligibility(current, semantic.nativeValues, scopes),
          }),
        ),
        annotations: Transaction.addToHistory.of(false),
      });
    });
  }
}

/** Key settlement and composition are observed through supported CodeMirror lifecycle APIs. */
export const loraSchedulePreparation = ViewPlugin.fromClass(LoraSchedulePreparation, {
  eventObservers: {
    /** Physical key release permits preparation outside its input cycle. */
    keyup() {
      this.queue();
    },
    /** Focus can move before physical key release. */
    blur() {
      this.queue();
    },
    /** Suspend preparation while the browser owns preedit. */
    compositionstart() {
      this.compositionStart();
    },
    /** Resume after CodeMirror flushes committed composition. */
    compositionend() {
      this.compositionEnd();
    },
  },
});
