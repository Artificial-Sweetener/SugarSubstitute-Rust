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
/** One CodeMirror state field owns active prompt suggestions and their selected index, independently of viewport DOM. */
import { requireValue } from '../../core/src/required.js';
import { StateEffect, StateField, type EditorState, type Transaction } from '@codemirror/state';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import type {
  CompletionQuery,
  PreparedCompletion,
  PreparedCompletionResult,
  CompletionActivation,
} from './prompt-completion-model.js';
import { semanticStructures } from './structure.js';
import {
  prepareNativeCompletionContinuation,
  followNativeCompletionContinuation,
  type NativeCompletionContinuation,
} from './document-value-continuation.js';
/** Native session identity binds derived geometry, media and accessible option IDs to immutable prepared candidates. */
export interface CompletionSession {
  readonly id: string;
  readonly result: PreparedCompletionResult;
  readonly selected: number;
  readonly phase: 'ready' | 'refreshing';
  readonly query: CompletionQuery;
  readonly freshness: EditorFreshness;
  readonly caret: number;
  readonly acceptance: {
    readonly identity: string;
    readonly activation: CompletionActivation;
  } | null;
  readonly presentation: symbol;
  /** Logical UTF-16 size change inside the prepared replacement's consumed content; null requires fresh semantics. */
  readonly consumedShift: number | null;
}
/** One authoritative native owner stores the visible session and its independently advancing executable receipt. */
interface CompletionState {
  readonly session: CompletionSession | null;
  readonly continuation: NativeCompletionContinuation | null;
  readonly selectedExplicitly: boolean;
}
let nextIdentity = 0;
/** Publish prepared source-current candidates or explicitly dismiss; no source parsing occurs in the field. */
export const installCompletion = StateEffect.define<PreparedCompletionResult | null>();
/** Selection intent is bound to the currently installed candidates and never owns the editable caret. */
export const selectCompletion = StateEffect.define<{
  readonly result: PreparedCompletionResult;
  readonly index: number;
}>();
/** Retain one explicit acceptance intent while current source semantics are still being prepared. */
export const requestCompletionAcceptance = StateEffect.define<{
  readonly result: PreparedCompletionResult;
  readonly index: number;
  readonly activation: CompletionActivation;
}>();
/** Validate source, context, caret and permission before either publication or reuse of prepared candidates. */
function current(state: EditorState, result: PreparedCompletionResult): boolean {
  const option = result.options[0];
  return (
    !!option &&
    !state.readOnly &&
    state.selection.main.empty &&
    option.freshness === state.field(editorFreshness) &&
    option.caret === state.selection.main.head
  );
}
/** Candidate identity follows the exact installed model, including distinct formats with the same prompt name. */
export function completionIdentity(option: PreparedCompletion): string {
  return option.kind === 'tag'
    ? `tag:${option.label}`
    : option.kind === 'trigger'
      ? `trigger:${option.backendValue}:${option.label}`
      : option.kind === 'wildcard'
        ? `wildcard:${option.label}`
        : option.kind === 'scene'
          ? `scene:${option.label}`
          : `lora:${option.item.model.backendValue}`;
}
/** Retain prepared rows provisionally for one native content edit; prepared query publication owns semantic compatibility. */
function followInput(
  session: CompletionSession,
  transaction: Transaction,
  executable: boolean,
): CompletionSession | null {
  const state = transaction.state,
    freshness = state.field(editorFreshness);
  if (
    state.readOnly ||
    !state.selection.main.empty ||
    freshness.context !== session.freshness.context ||
    !transaction.docChanged ||
    (!transaction.isUserEvent('input.type') && !transaction.isUserEvent('delete')) ||
    transaction.isUserEvent('input.type.compose')
  )
    return null;
  let count = 0,
    inside = true;
  transaction.changes.iterChangedRanges((from, to) => {
    count++;
    if (from < session.query.contentFrom || to > session.query.to) inside = false;
  });
  const caret = transaction.changes.mapPos(session.caret, 1),
    end = transaction.changes.mapPos(session.query.to, 1);
  if (
    count !== 1 ||
    !inside ||
    state.selection.main.head !== caret ||
    caret < session.query.contentFrom ||
    caret > end
  )
    return null;
  return {
    ...session,
    phase: 'refreshing',
    freshness,
    caret,
    acceptance: null,
    consumedShift: executable
      ? (session.consumedShift ?? 0) + transaction.newDoc.length - transaction.startState.doc.length
      : null,
    query: { ...session.query, to: end },
  };
}
/** Compatible source refinement preserves the visible row; ready metadata publication follows ranking unless selection was explicit. */
function publish(
  session: CompletionSession | null,
  result: PreparedCompletionResult,
  transaction: Transaction,
  selectedExplicitly: boolean,
): CompletionState {
  const state = transaction.state;
  const compatible =
    session &&
    session.query.kind === result.query.kind &&
    (session.query.scope === result.query.scope ||
      (session.query.scope.container.location.kind === 'prompt' &&
        result.query.scope.container.location.kind === 'prompt')) &&
    session.query.from === result.query.from &&
    session.query.contentFrom === result.query.contentFrom;
  const previous =
    compatible && (selectedExplicitly || session.phase === 'refreshing')
      ? session.result.options[session.selected]
      : null;
  const selected = previous
    ? result.options.findIndex(
        (option) => completionIdentity(option) === completionIdentity(previous),
      )
    : -1;
  const samePresentation =
    compatible &&
    session.result.options.length === result.options.length &&
    result.options.every((option, index) => {
      const previous = requireValue(session.result.options[index], 'completion-session');
      return (
        option.kind === previous.kind &&
        option.label === previous.label &&
        option.detail === previous.detail &&
        (option.kind !== 'lora' || (previous.kind === 'lora' && option.item === previous.item))
      );
    });
  const origin = transaction.startState,
    native = origin.field(semanticStructures).native,
    target = result.options[0]?.acceptance.keyboard.edit.target;
  let continuation: NativeCompletionContinuation | null = null;
  if (native && target) {
    const prepared = prepareNativeCompletionContinuation(
      origin,
      {
        context: {
          values: native.nativeValues,
          semantics: native.documentSemantics,
          enabled: true,
          composing: false,
        },
        target,
      },
      result,
    );
    if (prepared.status === 'ready') {
      const followed = followNativeCompletionContinuation(prepared.value, transaction);
      if (followed.status === 'ready') continuation = followed.value.continuation;
    }
  }
  return {
    session: {
      id: compatible ? session.id : `sp-completions-${++nextIdentity}`,
      result,
      selected: Math.max(0, selected),
      phase: 'ready',
      query: result.query,
      freshness: state.field(editorFreshness),
      caret: state.selection.main.head,
      acceptance: null,
      presentation: samePresentation ? session.presentation : Symbol(),
      consumedShift: 0,
    },
    continuation,
    selectedExplicitly: !!compatible && selectedExplicitly && selected >= 0,
  };
}
/** Native state retains rows across bounded input and retires on unrelated source/context/caret changes. */
const completionState = StateField.define<CompletionState>({
  /** Baselines begin without an active suggestion session. */
  create() {
    return { session: null, continuation: null, selectedExplicitly: false };
  },
  /** Apply only source-current publication and bounded, same-session selection intent. */
  update(previous, transaction) {
    let session = previous.session;
    let continuation = previous.continuation;
    let selectedExplicitly = previous.selectedExplicitly;
    if (continuation) {
      const followed = followNativeCompletionContinuation(continuation, transaction);
      continuation = followed.status === 'ready' ? followed.value.continuation : null;
    }
    if (
      session &&
      (session.freshness !== transaction.state.field(editorFreshness) ||
        session.caret !== transaction.state.selection.main.head ||
        !transaction.state.selection.main.empty ||
        transaction.state.readOnly)
    )
      session = followInput(session, transaction, continuation !== null);
    if (!session) {
      continuation = null;
      selectedExplicitly = false;
    }
    for (const effect of transaction.effects) {
      if (effect.is(installCompletion)) {
        const result = effect.value;
        if (!result) {
          session = null;
          continuation = null;
          selectedExplicitly = false;
        } else if (current(transaction.state, result)) {
          const published = publish(session, result, transaction, selectedExplicitly);
          session = published.session;
          continuation = published.continuation;
          selectedExplicitly = published.selectedExplicitly;
        }
      } else if (effect.is(selectCompletion) && session && effect.value.result === session.result) {
        const index = effect.value.index;
        if (Number.isSafeInteger(index) && index >= 0 && index < session.result.options.length) {
          selectedExplicitly = true;
          if (index !== session.selected || session.acceptance !== null)
            session = { ...session, selected: index, acceptance: null };
        }
      } else if (
        effect.is(requestCompletionAcceptance) &&
        session?.phase === 'refreshing' &&
        effect.value.result === session.result
      ) {
        const option = Number.isSafeInteger(effect.value.index)
          ? session.result.options[effect.value.index]
          : undefined;
        if (option) {
          selectedExplicitly = true;
          session = {
            ...session,
            selected: effect.value.index,
            acceptance: {
              identity: completionIdentity(option),
              activation: effect.value.activation,
            },
          };
        }
      }
    }
    return session === previous.session &&
      continuation === previous.continuation &&
      selectedExplicitly === previous.selectedExplicitly
      ? previous
      : { session, continuation, selectedExplicitly };
  },
});
/** The public native session observation preserves the exact visible object while its owner advances private transaction evidence. */
export const completionSession = StateField.define<CompletionSession | null>({
  /** Observe the initial authoritative session supplied by the same extension provision. */
  create: (state) => state.field(completionState).session,
  /** Observe the authoritative owner without reproducing publication, selection or continuation rules. */
  update: (_previous, transaction) => transaction.state.field(completionState).session,
  /** Include the one authority whenever a consumer mounts the visible session observation. */
  provide: () => completionState,
});
/** Acceptance reads only the authoritative current receipt; visible row identity does not confer executable authority. */
export function completionContinuation(state: EditorState): NativeCompletionContinuation | null {
  return state.field(completionState, false)?.continuation ?? null;
}
/** Read the sole selected native suggestion; painting and acceptance never maintain a parallel selected row. */
export function selectedCompletion(state: EditorState): PreparedCompletion | null {
  const session = state.field(completionSession);
  return session?.result.options[session.selected] ?? null;
}
