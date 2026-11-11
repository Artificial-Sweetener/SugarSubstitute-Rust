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
/** One finite native weight command queue delegates semantics and canonical source/caret/history to their owners. */
import { requireValue } from '../../core/src/required.js';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { captureFocusHandoff, currentFocusHandoff } from './focus-handoff.js';
import {
  Transaction,
  Prec,
  type Extension,
  type StateEffect,
  type EditorState,
} from '@codemirror/state';
import { ViewPlugin, keymap, type EditorView, type ViewUpdate } from '@codemirror/view';
import { exactWeightDraft, setExactWeightDraft } from './exact-weight-draft.js';
import { richRendering } from './presentation.js';
import { editorFreshness } from './freshness.js';
import { nativeInputSettlement, type CommandInputSettlement } from './input-settlement.js';
import {
  prepareWeightChange,
  type WeightRequest,
  type SpatialWeightRequest,
} from './weight-command.js';
import { weightEditingAllowed, steppableLabelSelector } from './weight-target.js';
import type { WeightChange } from '../../core/src/prompt/weight-mutations.js';
import {
  weightSession,
  retireWeightSession,
  installWeightSession,
  type WeightSessionPublication,
} from './weight-session.js';
/** Queue-only wheel authority stays with its mounted interaction owner; pure semantic requests retain no view callback. */
type QueuedWeightRequest =
  | Exclude<
      WeightRequest,
      {
        readonly owner: 'wheel';
      }
    >
  | (Extract<
      WeightRequest,
      {
        readonly owner: 'wheel';
      }
    > & {
      readonly current: () => boolean;
    });
/** Delegate only wheel lifetime admission; keyboard, pointer and exact commands keep their existing authorities. */
function currentQueuedWeight(request: QueuedWeightRequest): boolean {
  return request.owner !== 'wheel' || request.current();
}
/** Recognize the exact Ctrl+Up/Down authoring intent while pure modifier keys preserve a continuing session. */
function isWeightKey(event: KeyboardEvent): boolean {
  return (
    event.ctrlKey &&
    !event.altKey &&
    !event.metaKey &&
    !event.shiftKey &&
    (event.key === 'ArrowUp' || event.key === 'ArrowDown')
  );
}
/** The native view owns queued work; a job retains identities/selection observations, never a competing text buffer. */
class WeightCommands {
  private generation = 0;
  private disposed = false;
  private timer: number | null = null;
  private pending: {
    readonly change: WeightChange | null;
    readonly request: QueuedWeightRequest;
    readonly handoff: FocusHandoff;
    readonly draftKey: symbol | null;
  }[] = [];
  private publishing = false;
  private settlement: CommandInputSettlement | null = null;
  private readonly view: EditorView;
  /** Bind the queue to one mounted CodeMirror lifetime. */
  constructor(view: EditorView) {
    this.view = view;
  }
  /** Cancel queued work before the retired view can receive another publication. */
  destroy(): void {
    this.disposed = true;
    this.cancel();
  }
  /** Native source, permissions, catalogs or presentation replacement supersede every pending request. */
  update(update: ViewUpdate): void {
    if (
      !this.publishing &&
      (update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
        (!this.outsideCompletion() && !update.startState.selection.eq(update.state.selection)) ||
        update.startState.facet(richRendering) !== update.state.facet(richRendering))
    )
      this.cancel();
  }
  /** Capture intent before native callbacks, or retain the exact draft's pre-close handoff; preparation preserves source/context/selection freshness. */
  request(
    change: WeightChange | null,
    request: QueuedWeightRequest = { owner: 'keyboard' },
    handoff?: FocusHandoff,
    draftKey: symbol | null = null,
  ): boolean {
    const state = this.view.state;
    if (
      this.disposed ||
      !weightEditingAllowed(state, request.owner === 'keyboard' ? undefined : request.kind) ||
      this.view.compositionStarted ||
      this.view.composing ||
      !currentQueuedWeight(request)
    )
      return false;
    this.pending.push({
      change,
      request,
      handoff: handoff ?? captureFocusHandoff(this.view),
      draftKey,
    });
    if (this.settlement !== null) return true;
    this.settlement = state.facet(nativeInputSettlement).reserveCommand(this.view);
    const generation = this.generation;
    const freshness = state.field(editorFreshness),
      selection = state.selection;
    /** Observe source/context/selection freshness before accepting measured native readiness. */
    const current = (): boolean =>
      !this.disposed &&
      generation === this.generation &&
      this.view.state.field(editorFreshness) === freshness &&
      (this.outsideCompletion() || this.view.state.selection.eq(selection)) &&
      !this.view.compositionStarted &&
      !this.view.composing &&
      this.pending.every((intent) => currentQueuedWeight(intent.request)) &&
      !this.disposed &&
      generation === this.generation;
    // Native event listeners have intervening microtask checkpoints; a task lets all host reactions finish first.
    this.timer = window.setTimeout(() => {
      this.timer = null;
      if (!current()) {
        if (generation === this.generation) this.cancel();
        return;
      }
      this.view.requestMeasure({
        key: this,
        /** Only a current command may read the mounted native viewport anchor. */
        read: () => (current() ? this.view.scrollSnapshot() : null),
        /** Finish accepted typing and resolve commands after all read/write phases. */
        write: (scroll) =>
          queueMicrotask(() => {
            if (scroll && current()) this.execute(generation, scroll);
            else if (generation === this.generation) this.cancel();
          }),
      });
    }, 0);
    return true;
  }
  /** Capture a real draft before callbacks; keyboard acceptance publishes before native focus and the next key. */
  exact(
    change: WeightChange | null,
    request: Extract<
      SpatialWeightRequest,
      {
        owner: 'exact';
      }
    >,
    handoff: FocusHandoff,
    key: symbol,
  ): boolean {
    const draft = this.view.state.field(exactWeightDraft);
    if (
      draft?.key !== key ||
      draft.phase !== 'editing' ||
      draft.freshness !== request.freshness ||
      draft.kind !== request.kind ||
      draft.range.from !== request.range.from ||
      draft.range.to !== request.range.to
    )
      return false;
    if (!this.request(change, request, handoff, key)) return false;
    const generation = this.generation;
    try {
      this.view.dispatch({
        effects: setExactWeightDraft.of({ ...draft, phase: 'awaiting-commit' }),
        annotations: Transaction.addToHistory.of(false),
      });
    } catch (cause) {
      try {
        this.cancel();
      } catch (cleanup) {
        throw new AggregateError([cause, cleanup], 'Exact weight phase cleanup failed');
      }
      throw cause;
    }
    if (generation !== this.generation || this.disposed) return true;
    if (request.completion !== 'outside') {
      if (this.timer !== null) window.clearTimeout(this.timer);
      this.timer = null;
      this.execute(generation, null);
    }
    return true;
  }
  /** Retire only the captured numeric cell; view-update cancellation defers dispatch until update completes. */
  private settleDraft(key: symbol | null, deferred: boolean): void {
    if (key === null) return;
    const settle = (): void => {
      if (!this.disposed && this.view.state.field(exactWeightDraft)?.key === key)
        this.view.dispatch({
          effects: setExactWeightDraft.of(null),
          annotations: Transaction.addToHistory.of(false),
        });
    };
    if (deferred) queueMicrotask(settle);
    else settle();
  }
  /** Consume the reserved input boundary once, then retain every ordered command against the resulting native source. */
  private execute(generation: number, scroll: StateEffect<unknown> | null): void {
    const pending = this.pending,
      settlement = this.settlement;
    this.pending = [];
    this.settlement = null;
    this.publishing = true;
    let acceptedState: EditorState | null = null;
    let spatialTarget: SpatialWeightRequest | null = null;
    let restoration: FocusHandoff | null = null;
    const handoff = pending.at(-1)?.handoff;
    /** The latest admitted repeat owns the ordered batch; callbacks choosing another intent retire its remainder. */
    const current = (): boolean =>
      !this.disposed &&
      generation === this.generation &&
      handoff !== undefined &&
      currentFocusHandoff(this.view, handoff) &&
      pending.every((intent) => currentQueuedWeight(intent.request)) &&
      !this.disposed &&
      generation === this.generation &&
      (acceptedState !== null ||
        pending.every(
          (intent) =>
            intent.draftKey === null ||
            (this.view.state.field(exactWeightDraft)?.key === intent.draftKey &&
              this.view.state.field(exactWeightDraft)?.phase === 'awaiting-commit'),
        ));
    try {
      if (!current()) {
        settlement?.cancel();
        return;
      }
      settlement?.finish();
      for (const intent of pending) {
        if (!current() || (acceptedState !== null && this.view.state !== acceptedState)) break;
        let request = intent.request;
        const session = this.view.state.field(weightSession);
        if (
          request.owner !== 'keyboard' &&
          spatialTarget !== null &&
          request.freshness === spatialTarget.freshness &&
          request.kind === spatialTarget.kind &&
          request.range.from === spatialTarget.range.from &&
          request.range.to === spatialTarget.range.to &&
          session?.owner === request.owner &&
          session.kind === request.kind &&
          spatialTarget.owner === request.owner &&
          (request.owner !== 'wheel' ||
            (spatialTarget.owner === 'wheel' && request.token === spatialTarget.token))
        ) {
          // Only this batch's accepted step can advance its captured target; host edits still retire the batch.
          request = {
            ...request,
            freshness: this.view.state.field(editorFreshness),
            range: session.range,
          };
        }
        if (
          request.owner === 'exact' &&
          request.freshness !== this.view.state.field(editorFreshness)
        )
          continue;
        let transactions = intent.change
          ? prepareWeightChange(this.view.state, intent.change, request)
          : [];
        if (!transactions.length && request.owner !== 'exact') continue;
        if (scroll && intent.request.owner !== 'keyboard' && transactions.length) {
          const mapped = scroll.map(requireValue(transactions[0], 'weight-interactions').changes);
          if (mapped) {
            scroll = mapped;
            transactions = [
              ...transactions,
              requireValue(transactions.at(-1), 'weight-interactions').state.update({
                effects: mapped,
                annotations: Transaction.addToHistory.of(false),
              }),
            ];
          }
        }
        if (transactions.length) {
          if (!current()) break;
          acceptedState = requireValue(transactions.at(-1), 'weight-interactions').state;
          this.view.dispatch(transactions);
          if (
            !current() ||
            this.view.state !== requireValue(transactions.at(-1), 'weight-interactions').state
          )
            break;
        }
        acceptedState = this.view.state;
        spatialTarget = intent.request.owner !== 'keyboard' ? intent.request : null;
        if (request.owner === 'exact' && current() && this.view.state === acceptedState) {
          if (this.view.state.field(weightSession)?.owner === 'exact') {
            const transaction = this.view.state.update({
              effects: retireWeightSession.of(null),
              annotations: Transaction.addToHistory.of(false),
            });
            this.view.dispatch(transaction);
            if (!current() || this.view.state !== transaction.state) break;
            acceptedState = this.view.state;
          }
          if (request.completion === 'space') {
            const transaction = this.view.state.update(this.view.state.replaceSelection(' '), {
              userEvent: 'input.type',
            });
            this.view.dispatch(transaction);
            if (!current() || this.view.state !== transaction.state) break;
            acceptedState = this.view.state;
          }
        }
        // Focus dispatch is terminal: native reveal/readiness cannot interrupt accepted source continuation.
        if (
          current() &&
          this.view.state === acceptedState &&
          (request.owner === 'pointer' ||
            (request.owner === 'exact' && request.completion !== 'outside'))
        )
          restoration = intent.handoff;
      }
      if (current() && this.view.state === acceptedState && restoration)
        void restoration.restore('preserve-viewport');
    } finally {
      for (const intent of pending) this.settleDraft(intent.draftKey, false);
      this.publishing = false;
    }
  }
  /** Explicit typing/navigation/pointer/focus intent retires continuation without changing native source or history. */
  retire(): void {
    this.cancel();
    if (!this.disposed && this.view.state.field(weightSession))
      this.view.dispatch({
        effects: retireWeightSession.of(null),
        annotations: Transaction.addToHistory.of(false),
      });
  }
  /** Native numeric input temporarily owns focus while retaining its explicit source target. */
  focusLost(): void {
    if (this.view.state.field(weightSession)?.owner !== 'exact') this.retire();
  }
  /** A captured outside numeric commit survives the pointer selection that deliberately finalized it. */
  outsideCompletion(): boolean {
    return (
      this.pending.length > 0 &&
      this.pending.every(
        (intent) => intent.request.owner === 'exact' && intent.request.completion === 'outside',
      )
    );
  }
  /** Retire only the specified spatial owner and its derived continuation. */
  cancelTarget(owner: SpatialWeightRequest['owner']): void {
    if (this.pending.some((intent) => intent.request.owner === owner)) this.cancel();
    if (!this.disposed && this.view.state.field(weightSession)?.owner === owner)
      this.view.dispatch({
        effects: retireWeightSession.of(null),
        annotations: Transaction.addToHistory.of(false),
      });
  }
  /** Retire queued adjustment before the input owner publishes its source-current transient target. */
  beginExact(
    value: WeightSessionPublication & {
      readonly owner: 'exact';
    },
  ): void {
    this.cancel();
    this.view.dispatch({
      effects: installWeightSession.of(value),
      annotations: Transaction.addToHistory.of(false),
    });
  }
  /** Release the one task and its ordered intents on native or host supersession. */
  private cancel(): void {
    this.generation++;
    if (this.timer !== null) window.clearTimeout(this.timer);
    this.timer = null;
    const pending = this.pending;
    this.pending = [];
    for (const intent of pending) this.settleDraft(intent.draftKey, true);
    const settlement = this.settlement;
    this.settlement = null;
    settlement?.cancel();
  }
}
/** Install native keyboard routing and session lifetime observation through supported CodeMirror extension APIs. */
const plugin = ViewPlugin.fromClass(WeightCommands, {
  eventObservers: {
    /** Ordinary authoring/navigation ends the prior weight session; modifiers and adjustment repeats retain it. */
    keydown(event) {
      if (
        !isWeightKey(event) &&
        !['Shift', 'Control', 'Alt', 'Meta', 'AltGraph'].includes(event.key)
      )
        this.retire();
    },
    /** A deliberate pointer gesture starts a different editing intent. */
    mousedown(event) {
      if (this.outsideCompletion()) return;
      if (event.target instanceof Element && event.target.closest(steppableLabelSelector)) return;
      this.retire();
    },
    /** Focus moving to a host control releases the adjustment target. */
    blur() {
      this.focusLost();
    },
    /** Unfinished native preedit must not publish source commands or temporary feedback. */
    compositionstart() {
      this.retire();
    },
  },
});
/** Spatial controls delegate every measured source command to the same finite queue as keyboard intent. */
export function queueTargetWeight(
  view: EditorView,
  direction: 'increase' | 'decrease',
  request: Exclude<
    QueuedWeightRequest,
    {
      readonly owner: 'keyboard' | 'exact';
    }
  >,
): boolean {
  return view.plugin(plugin)?.request({ kind: 'step', direction }, request) ?? false;
}
/** Exact numeric completion carries its pre-close handoff into the finite source queue; draft teardown cannot manufacture newer focus intent. */
export function queueExactWeight(
  view: EditorView,
  weight: string | null,
  request: Extract<
    SpatialWeightRequest,
    {
      owner: 'exact';
    }
  >,
  handoff: FocusHandoff,
  key: symbol,
): boolean {
  return (
    view
      .plugin(plugin)
      ?.exact(weight === null ? null : { kind: 'set', weight }, request, handoff, key) ?? false
  );
}
/** Start a native numeric draft while retiring competing queued adjustment. */
export function beginExactWeight(
  view: EditorView,
  value: WeightSessionPublication & {
    readonly owner: 'exact';
  },
): void {
  view.plugin(plugin)?.beginExact(value);
}
/** A hidden or superseded spatial owner releases only its own queued work and transient session. */
export function cancelTargetWeight(view: EditorView, owner: SpatialWeightRequest['owner']): void {
  view.plugin(plugin)?.cancelTarget(owner);
}
/** Native keyboard routing installs the shared command lifetime and one adjustment state owner. */
export const keyboardWeightInteractions: Extension = [
  weightSession,
  plugin,
  Prec.high(
    keymap.of([
      {
        key: 'Ctrl-ArrowUp',
        run: (view) =>
          view.plugin(plugin)?.request({ kind: 'step', direction: 'increase' }) ?? false,
        preventDefault: true,
      },
      {
        key: 'Ctrl-ArrowDown',
        run: (view) =>
          view.plugin(plugin)?.request({ kind: 'step', direction: 'decrease' }) ?? false,
        preventDefault: true,
      },
    ]),
  ),
];
