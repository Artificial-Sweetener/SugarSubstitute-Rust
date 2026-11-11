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
/** Native history owns source snapshots; this extension supplies bounded derived key-edit grouping and intent boundaries. */
import { requireValue } from '../../core/src/required.js';
import {
  StateEffect,
  StateField,
  Transaction,
  EditorState,
  type EditorSelection,
  type Extension,
} from '@codemirror/state';
import { history, invertedEffects, isolateHistory } from '@codemirror/commands';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { unicodeAlphanumeric } from '../../core/src/prompt/alphanumeric.js';
import { changesExactSource } from './exact-source.js';
import { joinsPasteImportHistory, pasteImportOriginalSelection } from './paste-import-history.js';
import { normalizationJoin } from './input-descriptor.js';
import type { HostConfiguration } from './host-update.js';
/** One current key edit records only its category, resulting logical-document UTF-16 caret and native timestamp. */
interface KeyEdit {
  readonly kind: 'typing' | 'delete.backward' | 'delete.forward';
  readonly head: number;
  readonly time: number;
}
const boundary = StateEffect.define<null>();
/** Only the complete prepared publication may preserve typing across a neutral configuration change. */
const neutralConfiguration = StateEffect.define<readonly StateEffect<unknown>[]>();
/** History alone classifies host configuration intent; unknown/native reconfiguration remains a boundary. */
export function configurationHistoryIntent(
  changedKeys: readonly (keyof HostConfiguration)[],
  effects: readonly StateEffect<unknown>[],
): StateEffect<unknown> | null {
  const neutral: readonly (keyof HostConfiguration)[] = [
    'placeholder',
    'sourceLineChrome',
    'wheelMode',
    'tagLexicon',
    'tagCompletionCatalog',
    'loraCompletionCatalog',
    'wildcardCatalog',
    'sceneTitleCatalog',
    'sceneQueueHost',
    'loraPageHost',
    'loraMetadataHost',
    'loraSchedulingHost',
    'booruPostLookup',
    'spellcheckSession',
  ];
  return changedKeys.length && changedKeys.every((key) => neutral.includes(key))
    ? Object.freeze(neutralConfiguration.of(Object.freeze([...effects])))
    : null;
}
/** Added permission/composition/foreign effects cannot inherit the prepared neutral publication's intent. */
function preservesConfigurationHistory(transaction: Transaction): boolean {
  if (
    changesExactSource(transaction) ||
    transaction.selection ||
    transaction.annotation(isolateHistory)
  )
    return false;
  const intents = transaction.effects.filter((effect) => effect.is(neutralConfiguration));
  if (intents.length !== 1) return false;
  const intent = requireValue(intents[0], 'prompt-history');
  if (!intent.is(neutralConfiguration)) return false;
  const captured = intent.value;
  const actual = transaction.effects.filter((effect) => effect !== intent);
  return (
    actual.length === captured.length && actual.every((effect, index) => effect === captured[index])
  );
}
/** Actual native key/composition intent distinguishes a new physical key from late IME DOM flushes. */
const keyboardIntent = StateEffect.define<boolean>();
/** Native composition transport marks its first source change without retaining any preedit text. */
const compositionIntent = StateEffect.define<null>();
/** Only the first native multiline selection deletion precedes the browser's first preedit replacement. */
type CompositionPhase = 'inactive' | 'starting' | 'deleted-selection';
/** Recognize an exact native selection deletion in logical-document UTF-16 without scanning source. */
function deletesSelection(transaction: Transaction): boolean {
  const selection = transaction.startState.selection.main;
  if (selection.empty || transaction.startState.selection.ranges.length !== 1) return false;
  let deletions = 0,
    matches = false;
  transaction.changes.iterChanges((from, to, _newFrom, _newTo, inserted) => {
    deletions++;
    matches = from === selection.from && to === selection.to && inserted.length === 0;
  });
  return deletions === 1 && matches;
}
/** One derived native transport phase keeps the selected deletion and first preedit change in the same history event. */
const compositionPhase = StateField.define<CompositionPhase>({
  create: () => 'inactive',
  update(current, transaction) {
    for (const effect of transaction.effects) if (effect.is(compositionIntent)) return 'starting';
    if (!transaction.docChanged) return current;
    if (!transaction.isUserEvent('input.type.compose')) return 'inactive';
    return current === 'starting' && deletesSelection(transaction)
      ? 'deleted-selection'
      : 'inactive';
  },
});
/** Preserve each complete native transaction while classifying CodeMirror's initial multiline deletion as part of the same composition. */
const compositionInput = EditorState.transactionFilter.of((transaction) => {
  const phase = transaction.startState.field(compositionPhase);
  const event =
    phase === 'starting' &&
    transaction.annotation(Transaction.userEvent) === 'input' &&
    deletesSelection(transaction)
      ? 'input.type.compose.start'
      : phase === 'deleted-selection' && transaction.isUserEvent('input.type.compose.start')
        ? 'input.type.compose'
        : null;
  return event ? [{ userEvent: event }, transaction] : transaction;
});
/** One boolean observes input transport only; native composition, source, selection and history remain authoritative. */
const keyboardInput = StateField.define<boolean>({
  create: () => false,
  update(current, transaction) {
    if (transaction.reconfigured) return false;
    for (const effect of transaction.effects) if (effect.is(keyboardIntent)) current = effect.value;
    return current;
  },
});
/** Preserve the complete native transaction while marking a real subsequent key as ordinary typing, including Chromium's recent-composition flush label. */
const committedKeyInput = EditorState.transactionFilter.of((transaction) =>
  transaction.isUserEvent('input.type.compose') &&
  !transaction.isUserEvent('input.type.compose.start') &&
  transaction.startState.field(keyboardInput)
    ? [{ userEvent: 'input.type' }, transaction]
    : transaction,
);
/** Native history stores the actual associated selection in its inversion effects, in the restored logical document's UTF-16 coordinates. */
const restoreHistorySelection = StateEffect.define<EditorSelection>({
  map: (selection, changes) => selection.map(changes),
});
/** Match the pinned scalar word policy without scanning inserted bulk text or using the browser's newer Unicode tables. */
function wordCharacter(text: string): boolean {
  return (
    text.length <= 2 &&
    [...Array.from(text)].length === 1 &&
    (text === '_' || text === '-' || unicodeAlphanumeric(text))
  );
}
/** Observe only one native edit and its authoritative selection; bulk input, composition and semantic commands form separate events. */
function keyEdit(transaction: Transaction): KeyEdit | null {
  if (
    !transaction.docChanged ||
    transaction.isUserEvent('input.type.compose') ||
    transaction.startState.selection.ranges.length !== 1 ||
    transaction.newSelection.ranges.length !== 1 ||
    !transaction.newSelection.main.empty
  )
    return null;
  const selection = transaction.startState.selection.main;
  let result: KeyEdit | null = null,
    changes = 0;
  transaction.changes.iterChanges((from, to, _newFrom, newTo, inserted) => {
    changes++;
    let kind: KeyEdit['kind'] | null = null;
    if (
      transaction.isUserEvent('input.type') &&
      inserted.length <= 2 &&
      from === selection.from &&
      to === selection.to &&
      wordCharacter(inserted.toString())
    )
      kind = 'typing';
    else if (!inserted.length && from < to) {
      if (transaction.isUserEvent('delete.backward')) kind = 'delete.backward';
      else if (transaction.isUserEvent('delete.forward')) kind = 'delete.forward';
    }
    if (kind && transaction.newSelection.main.head === newTo)
      result = {
        kind,
        head: newTo,
        time: requireValue(transaction.annotation(Transaction.time), 'prompt-history'),
      };
  });
  return changes === 1 ? result : null;
}
/** Keep one derived current grouping descriptor; native history remains the sole source/selection restoration owner. */
const currentKeyEdit = StateField.define<KeyEdit | null>({
  create: () => null,
  update(current, transaction) {
    if (
      transaction.effects.some((effect) => effect.is(boundary)) ||
      (transaction.reconfigured && !preservesConfigurationHistory(transaction))
    )
      return null;
    if (transaction.annotation(normalizationJoin))
      return current ? { ...current, head: transaction.newSelection.main.head } : null;
    const isolation = transaction.annotation(isolateHistory);
    if (isolation === 'full' || isolation === 'after') return null;
    if (changesExactSource(transaction)) return keyEdit(transaction);
    return transaction.selection ? null : current;
  },
});
/** Adjacent words and direction-specific deletes join within the donor's actual idle windows; normalization stays with its accepted native input. */
function joinsCurrentEdit(transaction: Transaction): boolean {
  if (transaction.reconfigured && !preservesConfigurationHistory(transaction)) return false;
  if (transaction.annotation(normalizationJoin) || joinsPasteImportHistory(transaction))
    return true;
  const isolation = transaction.annotation(isolateHistory);
  if (isolation === 'full' || isolation === 'before') return false;
  const previous = transaction.startState.field(currentKeyEdit),
    current = keyEdit(transaction);
  if (
    !previous ||
    !current ||
    previous.kind !== current.kind ||
    previous.head !== transaction.startState.selection.main.head ||
    !transaction.startState.selection.main.empty
  )
    return false;
  const elapsed = current.time - previous.time;
  return elapsed >= 0 && elapsed < (current.kind === 'typing' ? 900 : 750);
}
/** Apply a native history-owned selection through an additional supported transaction in the same view update; later explicit selection intent wins. */
export function historyTransactions(transactions: readonly Transaction[]): readonly Transaction[] {
  let selection: EditorSelection | null = null;
  for (const transaction of transactions) {
    if (selection) selection = selection.map(transaction.changes);
    if (transaction.selection) selection = null;
    for (const effect of transaction.effects)
      if (effect.is(restoreHistorySelection)) selection = effect.value;
  }
  const last = transactions.at(-1);
  if (!selection || !last || last.state.selection.eq(selection)) return transactions;
  return [
    ...transactions,
    last.state.update({
      selection,
      annotations: Transaction.addToHistory.of(false),
      userEvent: 'select.history',
    }),
  ];
}
/** Capture one selection per native source history group; repeated characters, composition continuations and normalization do not grow its effect list. */
function invertSelection(transaction: Transaction) {
  if (
    !changesExactSource(transaction) ||
    transaction.annotation(normalizationJoin) ||
    (transaction.isUserEvent('input.type.compose') &&
      !transaction.isUserEvent('input.type.compose.start')) ||
    joinsCurrentEdit(transaction)
  )
    return [];
  return [
    restoreHistorySelection.of(
      pasteImportOriginalSelection(transaction) ?? transaction.startState.selection,
    ),
  ];
}
/** Prepare a source-neutral boundary for this current derived group; native history, caret and selection remain authoritative. */
export function historyBoundary(state: EditorState): Transaction | null {
  return state.field(currentKeyEdit)
    ? state.update({
        effects: boundary.of(null),
        annotations: Transaction.addToHistory.of(false),
      })
    : null;
}
/** One mounted native owner closes groups on actual command, pointer, composition and window-focus intent, including no-op navigation. */
class HistoryBoundaries {
  private readonly view: EditorView;
  private disposed = false;
  private readonly window: Window;
  /** Bind the real document window once; this owner has no timer, request, editable buffer or history stack. */
  constructor(view: EditorView) {
    this.view = view;
    const owner = view.dom.ownerDocument.defaultView;
    if (!owner) throw new Error('Native history window unavailable');
    this.window = owner;
    owner.addEventListener('blur', this.finish);
  }
  /** Observe key intent before native handling; pure modifiers and empty-selection Copy preserve a current typing word. */
  key(event: KeyboardEvent): void {
    const current = this.view.state.field(currentKeyEdit);
    if (
      event.isComposing ||
      event.key === 'Process' ||
      event.key === 'Dead' ||
      (event.target instanceof Element && event.target.closest('input,textarea'))
    )
      return;
    if (
      !this.view.composing &&
      ((!event.ctrlKey && !event.metaKey && !event.altKey) || event.getModifierState('AltGraph')) &&
      event.key.length <= 2 &&
      [...Array.from(event.key)].length === 1 &&
      !this.view.state.field(keyboardInput)
    )
      this.view.dispatch({ effects: keyboardIntent.of(true) });
    if (!current) return;
    if (event.key === 'Backspace' || event.key === 'Delete') {
      if (current.kind === (event.key === 'Backspace' ? 'delete.backward' : 'delete.forward'))
        return;
    } else if (current.kind === 'typing') {
      if (['Shift', 'Control', 'Alt', 'Meta', 'AltGraph'].includes(event.key)) return;
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === 'c')
        return;
      if (
        ((!event.ctrlKey && !event.metaKey && !event.altKey) ||
          event.getModifierState('AltGraph')) &&
        wordCharacter(event.key)
      )
        return;
    }
    this.finish();
  }
  /** Finish the current key group without separating deferred normalization from its accepted native edit. */
  readonly finish = (): void => {
    if (this.disposed) return;
    const transaction = historyBoundary(this.view.state);
    if (transaction) this.view.dispatch(transaction);
  };
  /** Start real native preedit with a separate event and retire the preceding physical-key intent. */
  compositionStart(): void {
    this.finish();
    if (this.disposed) return;
    this.view.dispatch({
      effects: [keyboardIntent.of(false), compositionIntent.of(null)],
    });
  }
  /** CodeMirror retires DOM observers; release this owner's one external window listener on view replacement or teardown. */
  destroy(): void {
    this.disposed = true;
    this.window.removeEventListener('blur', this.finish);
  }
}
/** Install one native history extension with word/deletion policy and finite intent observers. */
export const promptHistory: Extension = [
  compositionPhase,
  compositionInput,
  keyboardInput,
  committedKeyInput,
  currentKeyEdit,
  history({ newGroupDelay: 900, joinToEvent: joinsCurrentEdit }),
  invertedEffects.of(invertSelection),
  ViewPlugin.fromClass(HistoryBoundaries, {
    eventObservers: {
      /** No-op navigation still finishes the same current native group. */
      keydown(event) {
        this.key(event);
      },
      /** Pointer intent ends the old word before native selection or rich-widget input. */
      pointerdown() {
        this.finish();
      },
      /** Native menu intent is a boundary even if placement is subsequently retired. */
      contextmenu() {
        this.finish();
      },
      /** Leaving the actual editable surface ends the current key event group. */
      blur() {
        this.finish();
      },
      /** Composition owns its own native history event rather than extending a preceding typed word. */
      compositionstart() {
        this.compositionStart();
      },
    },
  }),
];
