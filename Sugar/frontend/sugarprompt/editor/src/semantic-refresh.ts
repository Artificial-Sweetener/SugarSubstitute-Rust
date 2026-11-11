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
/** Coalesce source-current semantic input preparation and measured rich rendering. */
import { duplicateDiagnostics } from './duplicate-diagnostics.js';
import { prepareProjection } from './projection-preparation.js';
import { Transaction, type EditorState } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
import { nativeInputSettlement, nativeCompositionStarted } from './input-settlement.js';
import { richRendering } from './presentation.js';
import { loraPresentation } from './lora-presentation.js';
import { loraMedia } from './lora-media-state.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { sceneErrors } from './scene-errors.js';
import { regionPalette } from './region-paint.js';
import { featureProfile } from './features.js';
import { conditioningContext } from './conditioning-context.js';
import { documentKind } from './document-kind.js';
import { nativeConditioning } from './conditioning-state.js';
import { prepareSemanticEffects } from './semantic-preparation.js';
import {
  installSemanticStructure,
  prepareNativeStructure,
  semanticStructures,
} from './structure.js';
import {
  installUnmeasuredProjection,
  currentUnmeasuredProjection,
  projectsRichSyntax,
  richProjection,
} from './rich-projection.js';

/** One captured native-history publication waits only for its actual physical key settlement. */
interface HistoryInputReservation {
  readonly view: EditorView;
  readonly state: EditorState;
  readonly generation: number;
  queued: boolean;
}

/** One native measurement owns every deferred preparation phase until completion or retirement. */
interface PreparationReservation {
  phase: 'requesting' | 'waiting' | 'reading' | 'deferred';
}

/** Finite native settlement and measurement reservations prepare source-current semantics and rich rendering. */
export class SemanticRefresh {
  private view: EditorView | null;
  private generation = 0;
  private queued: PreparationReservation | null = null;
  private composing = false;
  private historyReservation: HistoryInputReservation | null = null;
  private disposed = false;
  /** Prepare mounted source after CodeMirror establishes native layout and selection. */
  constructor(view: EditorView) {
    this.view = view;
    this.queue();
  }
  /** Pending callbacks become inert and release their view when the plugin is destroyed. */
  destroy(): void {
    this.disposed = true;
    this.generation++;
    this.view = null;
    this.historyReservation = null;
  }
  /** Source or presentation invalidation schedules one coalesced current-state preparation. */
  update(update: Pick<ViewUpdate, 'startState'> & Partial<Pick<ViewUpdate, 'transactions'>>): void {
    if (!this.measurementCurrent(update.startState)) {
      this.generation++;
      this.historyReservation = null;
      if (this.queued?.phase === 'waiting') this.queued = null;
    }
    if (
      this.view &&
      projectsRichSyntax(this.view.state) &&
      this.needsPreparation() &&
      update.transactions?.some(
        (transaction) => transaction.isUserEvent('undo') || transaction.isUserEvent('redo'),
      )
    )
      this.prepareHistoryInput();
    if (this.needsPreparation()) this.queue();
  }
  /** Capture actual undo/redo context once; no later notification renews its source authority. */
  private prepareHistoryInput(): void {
    const view = this.view;
    if (!view) return;
    const reservation: HistoryInputReservation = {
      view,
      state: view.state,
      generation: this.generation,
      queued: false,
    };
    this.historyReservation = reservation;
    this.resumeHistoryInput();
  }
  /** Resume only the exact retained reservation in an off-input microtask; active work coalesces settlement notifications. */
  private resumeHistoryInput(): void {
    const reservation = this.historyReservation;
    if (!reservation || reservation.queued) return;
    reservation.queued = true;
    queueMicrotask(() => {
      if (this.historyReservation !== reservation) return;
      reservation.queued = false;
      let waiting = false;
      try {
        const { view, state: captured, generation } = reservation;
        if (
          this.view !== view ||
          generation !== this.generation ||
          !this.measurementCurrent(captured)
        )
          return;
        const state = view.state;
        const readiness = this.inputReadiness(view, state, generation);
        if (readiness !== 'current') {
          waiting = readiness === 'held';
          return;
        }
        const semantic = prepareNativeStructure(state);
        const prepared = currentUnmeasuredProjection(state) ?? prepareProjection(state, semantic);
        const publication = this.inputReadiness(view, state, generation);
        if (publication !== 'current' || !this.measurementCurrent(captured)) {
          waiting = publication === 'held';
          return;
        }
        view.dispatch({
          effects: [
            installSemanticStructure.of({
              purpose: 'native',
              prepared: semantic,
            }),
            installUnmeasuredProjection.of(prepared),
          ],
          annotations: Transaction.addToHistory.of(false),
        });
      } finally {
        if (!waiting && this.historyReservation === reservation) this.historyReservation = null;
      }
    });
  }
  /** Preedit retains native text and excludes semantic discovery/publication. */
  compositionStart(): void {
    this.composing = true;
    this.historyReservation = null;
    this.generation++;
  }
  /** Follow CodeMirror's committed DOM flush before requesting preparation. */
  compositionEnd(): void {
    this.composing = false;
    queueMicrotask(() => queueMicrotask(() => this.queue()));
  }
  /** Follow the input owner's key-release notification, including focus moving before keyup. */
  keyEnd(): void {
    queueMicrotask(() => this.settleInput());
  }
  /** Settlement resumes only captured history; an unexpected input observation failure releases that exact reservation. */
  private settleInput(): void {
    const reservation = this.historyReservation;
    let completed = false;
    try {
      this.resumeHistoryInput();
      this.queue();
      completed = true;
    } finally {
      if (!completed && this.historyReservation === reservation) this.historyReservation = null;
    }
  }
  /** Pointer settlement prepares no syntax in its native event boundary. */
  pointerStart(): void {
    if (this.historyReservation) queueMicrotask(() => this.settleInput());
    else this.queue();
  }
  /** Boundary keys only request deferred work; native navigation consumes current prepared or raw source. */
  boundaryKey(event: Pick<KeyboardEvent, 'key' | 'ctrlKey'>): void {
    if (event.ctrlKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) return;
    if (
      /^(ArrowLeft|ArrowRight|ArrowUp|ArrowDown|Home|End|PageUp|PageDown|Backspace|Delete)$/u.test(
        event.key,
      )
    )
      this.queue();
  }
  /** Observe the admitted input owner once and distinguish only a genuine held-key wait from retirement after every reentry fence. */
  private inputReadiness(
    view: EditorView,
    state: EditorState,
    generation: number,
  ): 'current' | 'held' | 'retired' {
    if (
      this.disposed ||
      this.composing ||
      this.view !== view ||
      generation !== this.generation ||
      view.state !== state
    )
      return 'retired';
    const held = state.facet(nativeInputSettlement).hasHeldKey(view);
    const composition = nativeCompositionStarted(view);
    if (
      composition ||
      this.disposed ||
      this.composing ||
      (!held && nativeCompositionStarted(view)) ||
      this.view !== view ||
      generation !== this.generation ||
      view.state !== state
    )
      return 'retired';
    return held ? 'held' : 'current';
  }
  /** Existing measured publication consumes the same exact input observation and reentry fences. */
  private inputCurrent(view: EditorView, state: EditorState, generation: number): boolean {
    return this.inputReadiness(view, state, generation) === 'current';
  }
  /** Selection changes reuse the actual prepared decoration set and semantic snapshot. */
  private needsPreparation(): boolean {
    const state = this.view?.state;
    if (!state) return false;
    return (
      state.field(semanticStructures).native === null ||
      (state.field(duplicateDiagnostics, false) !== undefined &&
        state.field(duplicateDiagnostics).freshness !== state.field(editorFreshness)) ||
      (state.facet(documentKind) === 'prompt' && state.field(nativeConditioning) === null) ||
      (projectsRichSyntax(state) &&
        state.field(richProjection).sourceIdentity !== state.field(editorFreshness).source)
    );
  }
  /** Native selection can move after measurement; changed source, permissions or rendering inputs require another measurement. */
  private measurementCurrent(measured: EditorState): boolean {
    const current = this.view?.state;
    if (!current) return false;
    return (
      current.facet(documentKind) === measured.facet(documentKind) &&
      current.facet(featureProfile) === measured.facet(featureProfile) &&
      current.facet(nativeInputSettlement) === measured.facet(nativeInputSettlement) &&
      current.facet(conditioningContext) === measured.facet(conditioningContext) &&
      current.readOnly === measured.readOnly &&
      current.field(editorFreshness) === measured.field(editorFreshness) &&
      current.facet(richRendering) === measured.facet(richRendering) &&
      current.field(loraPresentation) === measured.field(loraPresentation) &&
      current.field(loraMedia) === measured.field(loraMedia) &&
      current.facet(wildcardCatalog) === measured.facet(wildcardCatalog) &&
      current.facet(regionPalette) === measured.facet(regionPalette) &&
      current.field(sceneErrors) === measured.field(sceneErrors)
    );
  }
  /** Measure native settlement first; preparation yields beyond the measurement frame. */
  private queue(): void {
    if (this.disposed || this.queued || this.composing || !this.needsPreparation()) return;
    const view = this.view;
    if (!view) return;
    const generation = this.generation,
      state = view.state;
    const reservation: PreparationReservation = {
      phase: 'requesting',
    };
    this.queued = reservation;
    let requested = false;
    let observed = false;
    try {
      const current = this.inputCurrent(view, state, generation);
      observed = true;
      if (!current) return;
      view.requestMeasure({
        key: this,
        /** Bind readiness to measured source/render inputs; normalization may publish a newer document before the deferred parser runs. */
        read: () => {
          if (this.queued !== reservation) return null;
          reservation.phase = 'reading';
          let completed = false;
          try {
            const currentView = this.view;
            const measured = currentView?.state;
            const result =
              currentView && measured && this.inputCurrent(currentView, measured, generation)
                ? measured
                : null;
            completed = true;
            return result;
          } finally {
            // CodeMirror skips write after a failed read; only that job's reservation retires.
            if (!completed && this.queued === reservation) this.queued = null;
          }
        },
        /** Yield a paint opportunity before CPU preparation, retaining the captured source and input fences. */
        write: (measured) => {
          if (this.queued !== reservation) return;
          reservation.phase = 'deferred';
          setTimeout(() => {
            let handedOff = false;
            try {
              const currentView = this.view;
              const currentState = currentView?.state;
              if (
                measured !== null &&
                currentView !== null &&
                currentState !== undefined &&
                this.inputCurrent(currentView, currentState, generation) &&
                this.measurementCurrent(measured) &&
                this.needsPreparation()
              ) {
                handedOff = true;
                this.prepare(currentView, currentState, generation, reservation);
              }
            } finally {
              if (!handedOff && this.queued === reservation) this.queued = null;
            }
            if (!handedOff) this.queue();
          }, 0);
        },
      });
      reservation.phase = 'waiting';
      requested = true;
    } finally {
      if (!requested) {
        if (this.queued === reservation) this.queued = null;
        if (
          observed &&
          !this.disposed &&
          this.view === view &&
          (generation !== this.generation || view.state !== state)
        )
          queueMicrotask(() => this.queue());
      }
    }
  }
  /** Yield large-source work between canonical phases and admit only one complete current publication. */
  private prepare(
    view: EditorView,
    state: EditorState,
    generation: number,
    reservation: PreparationReservation,
  ): void {
    const preparation = prepareSemanticEffects(state),
      interleave = state.doc.length > 8192;
    let resuming = false;
    /** Revalidate every resumed phase so typing, composition, configuration and disposal retire partial work. */
    const advance = (): void => {
      let waiting = false;
      try {
        if (
          !resuming ||
          (this.queued === reservation &&
            this.inputCurrent(view, state, generation) &&
            this.measurementCurrent(state))
        ) {
          let result = preparation.next();
          while (!interleave && !result.done) result = preparation.next();
          if (!result.done) {
            setTimeout(advance, 0);
            resuming = true;
            waiting = true;
          } else if (this.inputCurrent(view, state, generation) && this.measurementCurrent(state))
            view.dispatch({
              effects: result.value,
              annotations: Transaction.addToHistory.of(false),
            });
        }
      } finally {
        if (!waiting) {
          preparation.return([]);
          if (this.queued === reservation) this.queued = null;
        }
      }
      if (!waiting) this.queue();
    };
    advance();
  }
}

/** Native lifetime and composition notifications belong to the mounted CodeMirror view. */
export const semanticRefresh = ViewPlugin.fromClass(SemanticRefresh, {
  eventObservers: {
    /** Boundary keys schedule the finite measured job without synchronous semantic preparation. */
    keydown(event) {
      this.boundaryKey(event);
    },
    /** Rich publication waits for the same native key cycle as committed normalization. */
    keyup() {
      this.keyEnd();
    },
    /** Keyup may arrive in another control after focus moves. */
    blur() {
      this.keyEnd();
    },
    /** Schedule measured preparation before pointer hit testing, after the input owner retires its typing hold. */
    mousedown() {
      this.pointerStart();
    },
    /** Keep incomplete browser composition outside semantic preparation. */
    compositionstart() {
      this.compositionStart();
    },
    /** Refresh only after composition commits through CodeMirror. */
    compositionend() {
      this.compositionEnd();
    },
  },
});
