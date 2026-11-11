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
/** Native input records bounded edit descriptors; semantic preparation runs after committed input with freshness/lifetime guards. */
import { requireValue } from '../../core/src/required.js';
import { Transaction, type Extension } from '@codemirror/state';
import { EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view';
import { type TagLexicon } from '../../core/src/index.js';
import { authorship } from './authorship.js';
import { inputEdit, normalizationJoin, type PendingEdit } from './input-descriptor.js';
import { minimalChangedSourceRange, prepareNativeIngestionState } from './ingestion.js';
import {
  semanticStructures,
  prepareNativeStructure,
  installSemanticStructure,
} from './structure.js';
import { nativeDocumentValueAt } from './document-values.js';
import { prepareDocumentValueEdit } from './document-value-edit-issuance.js';
import { prepareDocumentValueTypedNormalization } from './document-value-ingestion.js';
import { dispatchNativeIngestion } from './native-ingestion.js';
import { authoredNesting } from './normalization-events.js';
import { featureProfile } from './features.js';
import { richRendering } from './presentation.js';
import { nativeInputSettlement, type CommandInputSettlement } from './input-settlement.js';
import { featureEnabled } from '../../services/src/index.js';
/** Own one queued semantic job and composition descriptor; disposal and supersession make queued callbacks inert. */
export class TypedNormalization {
  private readonly view: EditorView;
  private readonly lexicon: () => TagLexicon | undefined;
  private pending: PendingEdit | null = null;
  private composing = false;
  private readonly heldKeys = new Set<string>();
  private generation = 0;
  private disposed = false;
  private commandReservation: symbol | null = null;
  /** Bind one native view and the current immutable host lexicon observation. */
  constructor(view: EditorView, lexicon: () => TagLexicon | undefined) {
    this.view = view;
    this.lexicon = lexicon;
  }
  /** Retire queued preparation and release retained native document observations. */
  destroy(): void {
    this.disposed = true;
    this.pending = null;
    this.heldKeys.clear();
    this.commandReservation = null;
    this.generation++;
  }
  /** Native composition remains authoritative; only edit ranges accumulate during preedit. */
  compositionStart(): void {
    this.composing = true;
    this.pending = null;
    this.generation++;
  }
  /** Observe the same native input lifetime used to defer semantic preparation; this retains no preedit text. */
  hasCompositionInput(): boolean {
    return !this.disposed && this.composing;
  }
  /** Queue after CodeMirror's composition-end DOM flush microtask, then prepare final accepted source. */
  compositionEnd(): void {
    this.composing = false;
    const generation = ++this.generation;
    queueMicrotask(() =>
      queueMicrotask(() => {
        if (generation === this.generation) this.flush();
      }),
    );
  }
  /** A confirmed native cancellation retires its accumulated descriptor before semantic preparation can author source. */
  cancelCompositionInput(): void {
    this.pending = null;
    this.generation++;
  }
  /** Physical authoring keys own native DOM until their matching release; pure modifiers do not author content. */
  keyDown(event: Pick<KeyboardEvent, 'key' | 'code'>): void {
    if (['Shift', 'Control', 'Alt', 'Meta', 'AltGraph'].includes(event.key)) return;
    this.heldKeys.add(event.code || event.key);
  }
  /** Rendering observes this existing key lifetime instead of publishing during unfinished native input. */
  hasHeldKey(): boolean {
    return !this.disposed && this.heldKeys.size > 0;
  }
  /** Reserve the existing pending descriptor without parsing or mutating native input in the key handler. */
  reserveCommand(): CommandInputSettlement {
    const token = Symbol();
    this.commandReservation = token;
    this.generation++;
    return Object.freeze({
      /** Complete only this live reservation after the command owner measures native readiness. */
      finish: () => this.releaseCommand(token, true),
      /** Resume the sole input owner's ordinary preparation after command supersession. */
      cancel: () => this.releaseCommand(token, false),
    });
  }
  /** Settle once; disposal and replacement make stale handles inert. */
  private releaseCommand(token: symbol, finish: boolean): void {
    if (this.disposed || this.commandReservation !== token) return;
    this.commandReservation = null;
    this.generation++;
    if (finish) {
      this.heldKeys.clear();
      this.flush();
    } else if (this.pending && !this.composing) this.queue();
  }
  /** Release the matching physical key; blur or pointer intent retires the hold, and only a settled cycle schedules preparation. */
  keyUp(event: Pick<KeyboardEvent, 'key' | 'code'> | null): void {
    if (event) this.heldKeys.delete(event.code || event.key);
    else this.heldKeys.clear();
    if (!this.heldKeys.size && !this.composing && this.pending) this.queue();
  }
  /** Source/selection/read-only replacement retires stale work; native preedit maps its one accumulated interval. */
  update(update: Pick<ViewUpdate, 'startState' | 'state' | 'transactions'>): void {
    if (
      !update.state.facet(richRendering) ||
      update.startState.facet(richRendering) !== update.state.facet(richRendering)
    ) {
      this.pending = null;
      this.generation++;
      return;
    }
    const previousPending = this.pending;
    for (const transaction of update.transactions) {
      if (transaction.annotation(normalizationJoin)) continue;
      const edit = inputEdit(transaction);
      if (edit) {
        if (
          this.pending &&
          (this.composing ||
            edit.userEvent.startsWith('input.type.compose') ||
            (!edit.deletion &&
              !this.pending.deletion &&
              edit.from === transaction.changes.mapPos(this.pending.to, -1)))
        ) {
          const previous = this.pending,
            from = Math.min(edit.from, transaction.changes.mapPos(previous.from, -1)),
            to = Math.max(edit.to, transaction.changes.mapPos(previous.to, 1));
          this.pending = {
            ...previous,
            from,
            to,
            // Multiline composition starts with a selected-source deletion; its accepted range becomes insertion when preedit arrives.
            deletion: from === to,
            doc: edit.doc,
            selection: edit.selection,
            time: edit.time,
            userEvent: edit.userEvent,
          };
        } else this.pending = edit;
      } else if (transaction.docChanged || transaction.selection || transaction.state.readOnly) {
        this.pending = null;
        this.generation++;
      }
    }
    if (!this.composing && this.pending && this.pending !== previousPending) this.queue();
  }
  /** Native measurement first settles browser selection; preparation runs outside paint against the coalesced accepted range. */
  private queue(): void {
    if (this.commandReservation !== null) return;
    const generation = ++this.generation;
    if (this.pending?.manual) {
      queueMicrotask(() => {
        if (generation === this.generation) this.flush();
      });
      return;
    }
    this.view.requestMeasure({
      key: this,
      /** Observe job freshness after CodeMirror has flushed native DOM and selection changes. */
      read: () => !this.disposed && generation === this.generation,
      /** Semantic parsing stays outside CodeMirror's measurement and rendering phases. */
      write: (current) => {
        if (current)
          queueMicrotask(() => {
            if (generation === this.generation) this.flush();
          });
      },
    });
  }
  /** Prepare only semantic triggers; ordinary words retain native input without full source parsing. */
  private needsPreparation(edit: PendingEdit): boolean {
    if (edit.manual) return true;
    if (edit.protected) return false;
    const doc = this.view.state.doc,
      text = doc.sliceString(edit.from, edit.to);
    if (edit.deletion) {
      const line = doc.lineAt(edit.from);
      return (
        line.text.includes('[SEP') ||
        (line.number < doc.lines && doc.line(line.number + 1).text.includes('[SEP'))
      );
    }
    return (
      (text.length > 1 &&
        (/[()]/u.test(text) ||
          (text.includes(']') && doc.lineAt(edit.from).text.includes('[SEP')) ||
          (/[\d:.]$/u.test(text) && doc.toString().includes('\\(')))) ||
      (text.length === 1 &&
        (/[()]/u.test(text) ||
          (text === ']' && doc.lineAt(edit.from).text.includes('[SEP')) ||
          (/^[\d:.]$/u.test(text) && doc.toString().includes('\\(')))) ||
      (doc.lineAt(edit.from).from === edit.from && doc.sliceString(edit.to, edit.to + 4) === '[SEP')
    );
  }
  /** Prepare against current source and map native selection/provenance; one joined transaction publishes the result. */
  private flush(): void {
    const edit = this.pending;
    if (
      this.disposed ||
      this.commandReservation !== null ||
      (this.heldKeys.size > 0 && !edit?.manual) ||
      this.composing ||
      this.view.compositionStarted ||
      !edit
    )
      return;
    this.pending = null;
    const state = this.view.state;
    if (
      state.doc !== edit.doc ||
      !state.selection.eq(edit.selection) ||
      state.readOnly ||
      !state.facet(richRendering) ||
      !this.needsPreparation(edit)
    )
      return;
    const preparation = prepareNativeIngestionState(state),
      current = preparation.at(-1)?.state ?? state,
      native = requireValue(current.field(semanticStructures).native, 'typed-normalization');
    const context = {
      values: native.nativeValues,
      semantics: native.documentSemantics,
      enabled: true,
      composing: false,
    };
    const admitted = nativeDocumentValueAt(current, context, {
      from: edit.from,
      to: edit.to,
    });
    if (admitted.status === 'rejected') return;
    const target = admitted.target,
      mapping = requireValue(target.entry.semanticMapping, 'typed-normalization');
    const anchor =
        mapping.logicalToDecoded[state.selection.main.anchor - mapping.logicalRange.from],
      head = mapping.logicalToDecoded[state.selection.main.head - mapping.logicalRange.from];
    if (anchor == null || head == null) return;
    const lexicon = this.lexicon();
    const provenance = prepareDocumentValueTypedNormalization(
      target,
      target.decodedRange,
      current.field(authorship),
      {
        deletion: edit.deletion,
        manual: edit.manual,
        protected: edit.protected,
        ...(lexicon ? { lexicon } : {}),
      },
    );
    const normal = provenance.normalization,
      changed = minimalChangedSourceRange(target.scope.mapping.text, normal.text),
      depth =
        edit.profile === state.facet(featureProfile) && featureEnabled(edit.profile, 'emphasis')
          ? (authoredNesting(normal)[0]?.value ?? null)
          : null;
    const outcome = prepareDocumentValueEdit(current, {
      context,
      target,
      range: { from: changed.from, to: changed.beforeTo },
      text: normal.text.slice(changed.from, changed.afterTo),
      publication: {
        purpose: 'typed',
        normalization: provenance,
        input: {
          time: edit.time,
          userEvent: edit.userEvent.startsWith('input.type.compose')
            ? 'input.type.compose'
            : edit.userEvent,
          authoredImplicitDepth: depth,
        },
      },
      selection: {
        ranges: [
          {
            kind: 'decoded',
            anchor: requireValue(normal.boundaryPositions[anchor], 'typed-normalization'),
            head: requireValue(normal.boundaryPositions[head], 'typed-normalization'),
          },
        ],
        mainIndex: 0,
      },
    });
    if (outcome.status === 'rejected') throw new Error('decoded-typed-normalization-refused');
    const transactions = [...preparation, ...outcome.edit.transactions];
    const after = transactions.at(-1)?.state ?? state;
    if (after.field(semanticStructures).native === null)
      transactions.push(
        after.update({
          effects: installSemanticStructure.of({
            purpose: 'native',
            prepared: prepareNativeStructure(after, outcome.edit),
          }),
          annotations: Transaction.addToHistory.of(false),
        }),
      );
    if (!transactions.length) return;
    dispatchNativeIngestion(
      this.view,
      Object.freeze({
        startState: state,
        transactions: Object.freeze(transactions),
        edit: outcome.edit,
      }),
    );
  }
}
/** Install native lifecycle observation without intercepting browser input or owning a competing composition buffer. */
export function typedNormalization(lexicon: () => TagLexicon | undefined): Extension {
  const plugin = ViewPlugin.define((view) => new TypedNormalization(view, lexicon), {
    eventObservers: {
      /** Native key input completes before source-changing semantic follow-up. */
      keydown(event) {
        this.keyDown(event);
      },
      /** Release a held key without intercepting the browser or introducing another input buffer. */
      keyup(event) {
        this.keyUp(event);
      },
      /** Losing focus can route keyup elsewhere; release the input lifetime without moving focus back. */
      blur() {
        this.keyUp(null);
      },
      /** Explicit pointer intent finishes the previous typing hold; later physical repeats start a new cycle. */
      mousedown() {
        this.keyUp(null);
      },
      /** Record native composition lifetime without cancelling any event. */
      compositionstart() {
        this.compositionStart();
      },
      /** Native commit preparation follows CodeMirror's own pending DOM flush. */
      compositionend() {
        this.compositionEnd();
      },
    },
  });
  return [
    plugin,
    nativeInputSettlement.of({
      /** Public observations reuse the input owner's real platform start event. */
      hasCompositionInput: (view) => view.plugin(plugin)?.hasCompositionInput() ?? false,
      /** Delegate cancellation to the same owner of the one pending native input descriptor. */
      cancelCompositionInput: (view) => {
        const owner = view.plugin(plugin);
        if (!owner) throw new Error('Native input owner unavailable');
        owner.cancelCompositionInput();
      },
      /** Observe only the mounted input owner. */
      hasHeldKey: (view) => view.plugin(plugin)?.hasHeldKey() ?? false,
      /** Delegate finite reservation to that same owner. */
      reserveCommand: (view) => {
        const owner = view.plugin(plugin);
        if (!owner) throw new Error('Native input owner unavailable');
        return owner.reserveCommand();
      },
    }),
  ];
}
