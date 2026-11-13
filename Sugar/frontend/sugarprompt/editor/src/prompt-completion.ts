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
/** One native completion lifetime prepares cached prompt semantics and routes keys through CodeMirror state. */
import { requireValue } from '../../core/src/required.js';
import { Prec, Transaction, type Extension } from '@codemirror/state';
import { ViewPlugin, keymap, type EditorView, type ViewUpdate } from '@codemirror/view';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { semanticStructures } from './structure.js';
import { nativeInputSettlement } from './input-settlement.js';
import {
  completionSession,
  installCompletion,
  selectCompletion,
  selectedCompletion,
  completionIdentity,
  requestCompletionAcceptance,
} from './completion-session.js';
import { prepareCompletionOptions } from './completion-options.js';
import { acceptCompletionOption } from './prompt-completion-acceptance.js';
import { completionGhostProjection } from './completion-ghost.js';
import { completionTooltips } from './completion-tooltip.js';
import { completionWall } from './completion-wall-state.js';
import { loraWallVerticalIndex } from './lora-wall-layout.js';
import { loraPresentation } from './lora-presentation.js';
import { loraSchedulingHost, nativeLoraSchedule } from './lora-schedule.js';
import { featureEnabled } from '../../services/src/index.js';
import { featureProfile } from './features.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { loraCompletionCatalog, sceneTitleCatalog } from './prompt-completion-model.js';
/** Physical input and semantic readiness share one finite coalesced preparation, with no retained text buffer. */
export class CompletionPreparation {
  private pending: {
    readonly freshness: EditorFreshness;
    readonly caret: number;
  } | null = null;
  private queued = false;
  private disposed = false;
  private readonly view: EditorView;
  /** Mount inactive; baseline restoration never invents a user completion request. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Retire superseded work and refresh focused queries after native input or specialized catalog publication through the existing semantic owner. */
  update(update: ViewUpdate): void {
    if (
      this.pending &&
      (this.pending.freshness !== update.state.field(editorFreshness) ||
        this.pending.caret !== update.state.selection.main.head ||
        !update.state.selection.main.empty)
    )
      this.pending = null;
    if (update.focusChanged && !this.view.hasFocus) this.blur();
    if (
      update.docChanged &&
      update.transactions.some(
        (transaction) =>
          (transaction.isUserEvent('input') || transaction.isUserEvent('delete')) &&
          !transaction.isUserEvent('input.exact') &&
          !transaction.isUserEvent('input.complete'),
      )
    )
      this.pending = {
        freshness: update.state.field(editorFreshness),
        caret: update.state.selection.main.head,
      };
    if (
      (update.state.facet(loraCompletionCatalog) !==
        update.startState.facet(loraCompletionCatalog) ||
        update.state.facet(wildcardCatalog) !== update.startState.facet(wildcardCatalog) ||
        update.state.facet(sceneTitleCatalog) !== update.startState.facet(sceneTitleCatalog)) &&
      this.view.hasFocus &&
      !update.state.readOnly &&
      update.state.selection.main.empty
    )
      this.pending = {
        freshness: update.state.field(editorFreshness),
        caret: update.state.selection.main.head,
      };
    if (
      update.state.field(nativeLoraSchedule) !== update.startState.field(nativeLoraSchedule) &&
      update.state.field(completionSession) &&
      this.view.hasFocus
    )
      this.pending = {
        freshness: update.state.field(editorFreshness),
        caret: update.state.selection.main.head,
      };
    this.queue();
  }
  /** Native blur retires pending intent even when an intervening state update suppresses CM's deferred focus notification; refocus and teardown fence dismissal. */
  blur(): void {
    this.pending = null;
    queueMicrotask(() => {
      if (!this.disposed && !this.view.hasFocus) dismissCompletion(this.view);
    });
  }
  /** Destroyed views cannot publish queued options or retain a preparation epoch. */
  destroy(): void {
    this.disposed = true;
    this.pending = null;
  }
  /** Native preedit suppresses both current suggestions and retained preparation. */
  compositionStart(): void {
    this.pending = null;
    dismissCompletion(this.view);
  }
  /** Follow the existing native key/composition settlement before observing cached semantics. */
  resume(): void {
    this.queue();
  }
  /** Explicit dismissal/navigation retires preparation even before a tooltip has been published. */
  cancel(): void {
    this.pending = null;
  }
  /** Semantic preparation and physical key release converge on one microtask, never parsing during input/paint. */
  private queue(): void {
    if (this.disposed || this.queued || !this.pending) return;
    this.queued = true;
    this.view.requestMeasure({
      key: this,
      /** Native measurement flushes accepted DOM input before any asynchronous session publication. */
      read: () => !this.disposed,
      /** Candidate preparation follows native settlement outside measurement and paint. */
      write: (live) =>
        queueMicrotask(() => {
          this.queued = false;
          if (live && !this.disposed) this.prepare();
        }),
    });
  }
  /** Resolve the already prepared source slot and reject reentrant source/context replacement before publication. */
  private prepare(): void {
    const pending = this.pending,
      state = this.view.state;
    if (!pending) return;
    if (
      pending.freshness !== state.field(editorFreshness) ||
      pending.caret !== state.selection.main.head ||
      !state.selection.main.empty ||
      state.readOnly ||
      !this.view.hasFocus
    ) {
      this.pending = null;
      return;
    }
    if (
      this.view.compositionStarted ||
      this.view.composing ||
      state.facet(nativeInputSettlement).hasHeldKey(this.view) ||
      !state.field(semanticStructures).native
    )
      return;
    const result = prepareCompletionOptions(state);
    const resourceStatus = state.field(loraPresentation).snapshot.status,
      host = state.facet(loraSchedulingHost);
    if (
      !result &&
      featureEnabled(state.facet(featureProfile), 'lora_trigger_words') &&
      host.context?.supported &&
      host.context.catalogRevision === host.catalogRevision &&
      (!state.field(nativeLoraSchedule) ||
        resourceStatus === 'idle' ||
        resourceStatus === 'loading')
    )
      return;
    const previous = state.field(completionSession);
    const request = previous?.acceptance;
    if (this.pending !== pending || this.view.state.field(editorFreshness) !== pending.freshness)
      return;
    this.pending = null;
    this.view.dispatch({
      effects: installCompletion.of(result),
      annotations: Transaction.addToHistory.of(false),
    });
    const current = this.view.state.field(completionSession);
    const selected = selectedCompletion(this.view.state);
    if (
      request &&
      current &&
      previous?.id === current.id &&
      selected &&
      completionIdentity(selected) === request.identity
    )
      acceptCompletionOption(this.view, current.result, current.selected, request.activation);
  }
}
/** Dismiss only the native suggestion session; inactive keys fall through to ordinary editing. */
function dismissCompletion(view: EditorView): boolean {
  view.plugin(completionPreparation)?.cancel();
  if (!view.state.field(completionSession)) return false;
  view.dispatch({
    effects: installCompletion.of(null),
    annotations: Transaction.addToHistory.of(false),
  });
  return true;
}
/** Tab is the only key that accepts a suggestion; every other key stays ordinary text input. */
function acceptSelected(view: EditorView): boolean {
  const session = view.state.field(completionSession);
  if (!selectedCompletion(view.state) || !session) return false;
  if (acceptCompletionOption(view, session.result, session.selected, 'keyboard')) return true;
  if (session.phase === 'refreshing') {
    view.dispatch({
      effects: requestCompletionAcceptance.of({
        result: session.result,
        index: session.selected,
        activation: 'keyboard',
      }),
      annotations: Transaction.addToHistory.of(false),
    });
    return true;
  }
  return false;
}
/** Ordinary rows wrap; model linear navigation clamps and never changes the source caret. */
function moveSelection(
  view: EditorView,
  delta: number,
  modelsOnly = false,
  vertical = false,
): boolean {
  const session = view.state.field(completionSession);
  if (
    !session ||
    (modelsOnly && requireValue(session.result.options[0], 'prompt-completion').kind !== 'lora')
  )
    return false;
  const count = session.result.options.length;
  const wall = view.state.field(completionWall);
  if (
    vertical &&
    requireValue(session.result.options[0], 'prompt-completion').kind === 'lora' &&
    !wall
  )
    return true;
  const index =
    requireValue(session.result.options[0], 'prompt-completion').kind !== 'lora'
      ? (session.selected + delta + count) % count
      : vertical && wall
        ? loraWallVerticalIndex(wall.layout, session.selected, delta < 0 ? -1 : 1)
        : Math.max(0, Math.min(count - 1, session.selected + delta));
  view.dispatch({
    effects: selectCompletion.of({ result: session.result, index }),
    annotations: Transaction.addToHistory.of(false),
  });
  return true;
}
/** Native document/caret navigation continues after dismissing suggestions. */
function dismissAndNavigate(view: EditorView): boolean {
  dismissCompletion(view);
  return false;
}
/** The supported state-field/tooltip extension owns one prompt completion session and native key conventions. */
const completionPreparation = ViewPlugin.fromClass(CompletionPreparation, {
  eventObservers: {
    /** Actual content focus loss cannot retain suggestions or a queued acceptance request. */
    blur() {
      this.blur();
    },
    /** Preedit stays wholly native and excludes semantic acceptance. */
    compositionstart() {
      this.compositionStart();
    },
    /** Committed input follows CodeMirror's native composition flush. */
    compositionend() {
      queueMicrotask(() => queueMicrotask(() => this.resume()));
    },
    /** Physical key release lets prepared source queries proceed. */
    keyup() {
      this.resume();
    },
  },
});
/** One native state/session projects every prompt suggestion kind through supported CodeMirror extensions. */
export const promptCompletions: Extension = [
  completionSession,
  completionPreparation,
  completionTooltips,
  completionGhostProjection,
  Prec.highest(
    keymap.of([
      {
        key: 'ArrowDown',
        run: (view) => moveSelection(view, 1, false, true) || dismissAndNavigate(view),
      },
      {
        key: 'ArrowUp',
        run: (view) => moveSelection(view, -1, false, true) || dismissAndNavigate(view),
      },
      {
        key: 'ArrowLeft',
        run: (view) => moveSelection(view, -1, true) || dismissAndNavigate(view),
      },
      {
        key: 'ArrowRight',
        run: (view) => moveSelection(view, 1, true) || dismissAndNavigate(view),
      },
      { key: 'Tab', run: acceptSelected },
      { key: 'Escape', run: dismissCompletion },
      ...['Home', 'End', 'Enter'].map((key) => ({
        key,
        run: dismissAndNavigate,
      })),
    ]),
  ),
];
