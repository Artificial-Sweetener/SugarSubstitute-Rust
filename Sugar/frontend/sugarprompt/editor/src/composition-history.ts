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
/** Settle cancelled native composition through CodeMirror's own history and one finite immutable checkpoint. */
import { captureFocusHandoff } from './focus-handoff.js';
import {
  Compartment,
  Facet,
  Transaction,
  type EditorState,
  type Extension,
  type Text,
} from '@codemirror/state';
import { historyField, undo } from '@codemirror/commands';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import type { LineEnding } from '../../core/src/line-endings.js';
import { changesExactSource, lineEndings } from './exact-source.js';
import { nativeCompositionStarted, nativeInputSettlement } from './input-settlement.js';

/** The configuration identity is shared; its opaque history initializer and native values belong to each editor state. */
const historyInitialization = new Compartment();

/** The mounted surface alone owns complete native input retirement after a denied platform startup. */
export const compositionRetirement = Facet.define<
  (current: () => boolean) => void,
  ((current: () => boolean) => void) | null
>({
  combine: (values) => {
    if (values.length > 1) throw new Error('Multiple native input retirement owners');
    return values[0] ?? null;
  },
});

/** Retain native immutable values by reference for one active composition; no preedit or editable source buffer is stored. */
interface CompositionCheckpoint {
  readonly document: Text;
  readonly endings: readonly LineEnding[];
  /** CodeMirror's deliberately opaque public field value is restored through supported field initialization, never inspected or mutated. */
  readonly history: unknown;
}

/** Exact equality includes the original spelling of every logical newline. */
function matchesCheckpoint(state: EditorState, checkpoint: CompositionCheckpoint): boolean {
  const endings = state.field(lineEndings);
  return (
    state.doc.eq(checkpoint.document) &&
    endings.length === checkpoint.endings.length &&
    endings.every((ending, index) => ending === checkpoint.endings[index])
  );
}

/** Restore one opaque native history value and remove its initializer in the same native update, retaining current source and selection. */
export function compositionHistoryRestoration(
  state: EditorState,
  initialization: Compartment,
  history: unknown,
): readonly [Transaction, Transaction] {
  const restore = state.update({
    effects: initialization.reconfigure(historyField.init(() => history)),
    annotations: Transaction.addToHistory.of(false),
  });
  const release = restore.state.update({
    effects: initialization.reconfigure([]),
    annotations: Transaction.addToHistory.of(false),
  });
  return [restore, release];
}

/** One native plugin owns only a finite cancellation checkpoint and queued settlement generation. */
class CompositionHistory {
  private readonly view: EditorView;
  private checkpoint: CompositionCheckpoint | null = null;
  private generation = 0;
  /** Distinguish later platform starts from the cancellation's own history publications. */
  private startGeneration = 0;
  /** Only this exact native undo publication belongs to cancellation rather than a superseding source owner. */
  private restoration: Transaction | null = null;
  private disposed = false;

  /** The mounted native view and dedicated initializer compartment retain sole editing/history authority. */
  constructor(view: EditorView) {
    this.view = view;
  }

  /** Capture the actual native history value before the browser's first preedit mutation. */
  start(): void {
    const state = this.view.state;
    this.checkpoint = {
      document: state.doc,
      endings: state.field(lineEndings),
      history: state.field(historyField),
    };
    this.generation++;
    this.startGeneration++;
    if (state.readOnly) {
      const retire = state.facet(compositionRetirement),
        generation = this.generation;
      if (!retire) throw new Error('Native input retirement owner unavailable');
      retire(() => !this.disposed && generation === this.generation);
    }
  }

  /** Source changes outside native composition invalidate the checkpoint rather than rolling back a new owner. */
  update(update: ViewUpdate): void {
    if (
      update.transactions.some(
        (transaction) =>
          transaction !== this.restoration &&
          changesExactSource(transaction) &&
          transaction.annotation(Transaction.userEvent) !== 'input' &&
          !transaction.isUserEvent('input.type.compose'),
      )
    )
      this.retire();
  }

  /** Empty platform cancellation settles after CodeMirror's own queued DOM flush and before deferred semantic preparation. */
  end(event: CompositionEvent): void {
    const generation = this.generation;
    queueMicrotask(() =>
      queueMicrotask(() => {
        if (this.disposed || generation !== this.generation || this.view.compositionStarted) return;
        if (event.data === '') this.cancel();
        else this.retire();
      }),
    );
  }

  /** Prepare the existing native undo proposal and accept it only when it restores the owned immutable checkpoint. */
  private proposal(): Transaction | null {
    const checkpoint = this.checkpoint;
    if (!checkpoint || this.disposed || matchesCheckpoint(this.view.state, checkpoint)) return null;
    let proposed: Transaction | undefined;
    undo({
      state: this.view.state,
      dispatch: (transaction) => {
        proposed = transaction;
      },
    });
    return proposed && matchesCheckpoint(proposed.state, checkpoint) ? proposed : null;
  }
  /** Pure final-source/configuration admission consumes the same native cancellation proposal without dispatch or history mutation. */
  previewCancellation(): EditorState {
    return this.proposal()?.state ?? this.view.state;
  }

  /** Restore through native undo; an unfinished startup retains its one checkpoint until actual input completion or retirement. */
  cancel(continuingStartup = false): void {
    const checkpoint = this.checkpoint;
    const generation = ++this.generation;
    /** Reentrant source ownership, a later start and teardown invalidate remaining restoration publications. */
    const current = (): boolean => !this.disposed && generation === this.generation;
    if (!checkpoint || this.disposed) return;
    if (!matchesCheckpoint(this.view.state, checkpoint)) {
      const proposed = this.proposal();
      if (!proposed) {
        this.retire();
        return;
      }
      const previousRestoration = this.restoration;
      this.restoration = proposed;
      try {
        this.view.dispatch(proposed);
      } finally {
        this.restoration = previousRestoration;
      }
      if (!current()) return;
    }
    this.view.state.facet(nativeInputSettlement).cancelCompositionInput(this.view);
    if (!current()) return;
    this.view.dispatch(
      compositionHistoryRestoration(this.view.state, historyInitialization, checkpoint.history),
    );
    if (current() && !continuingStartup) this.checkpoint = null;
  }

  /** Admit native startup before CodeMirror marks composition and return a finite, start-specific retirement observation. */
  cancelForPermission(): (() => boolean) | null {
    if (nativeCompositionStarted(this.view) && !this.view.composing) {
      const startGeneration = this.startGeneration,
        generation = this.generation + 1;
      this.cancel(true);
      if (this.disposed) return null;
      return () =>
        !this.disposed &&
        generation === this.generation &&
        startGeneration === this.startGeneration;
    }
    this.closePlatform();
    return null;
  }

  /** Native focus departure ends platform input before rollback removes its DOM range; a newer host focus choice wins. */
  private closePlatform(): void {
    const startGeneration = this.startGeneration,
      refocus = nativeCompositionStarted(this.view) && this.view.hasFocus;
    const handoff = captureFocusHandoff(this.view, 'input');
    if (refocus) this.view.contentDOM.blur();
    if (this.disposed || startGeneration !== this.startGeneration) return;
    this.cancel();
    if (this.disposed || startGeneration !== this.startGeneration) return;
    const active = this.view.root.activeElement;
    if (refocus && (!active || active === this.view.dom.ownerDocument.body)) void handoff.restore();
  }

  /** Release every retained native value and make queued callbacks inert on supersession or teardown. */
  private retire(): void {
    this.checkpoint = null;
    this.generation++;
  }

  /** Native view teardown retires the checkpoint and its finite queued observation. */
  destroy(): void {
    this.disposed = true;
    this.retire();
  }
}

/** One plugin definition resolves the per-view cancellation owner without a side registry or retained editor handle. */
const nativeCompositionHistory = ViewPlugin.define((view) => new CompositionHistory(view), {
  eventObservers: {
    /** Observe the platform boundary without intercepting input. */
    compositionstart() {
      this.start();
    },
    /** Native input remains responsible for preedit and commit. */
    compositionend(event) {
      this.end(event);
    },
  },
});

/** Validate final source-neutral revocation inputs against the existing cancellation owner's projected native state. */
export function previewNativeCompositionCancellation(view: EditorView): EditorState {
  const owner = view.plugin(nativeCompositionHistory);
  if (!owner) throw new Error('Native composition owner unavailable');
  return owner.previewCancellation();
}

/** Restore unfinished native input while editing is enabled; a current observation requests complete retirement after native startup settles. */
export function cancelNativeComposition(view: EditorView): (() => boolean) | null {
  const owner = view.plugin(nativeCompositionHistory);
  if (!owner) throw new Error('Native composition owner unavailable');
  return owner.cancelForPermission();
}

/** Install finite native composition settlement before the input owner's deferred semantic completion observer. */
export const compositionHistory: Extension = [
  historyInitialization.of([]),
  nativeCompositionHistory,
];
