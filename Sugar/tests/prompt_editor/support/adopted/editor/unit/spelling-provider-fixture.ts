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
import { createNativeState } from '../../../../../../frontend/sugarprompt/editor/src/native-state.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
/** Finite actual-CM state/dispatch fixture controls only mounted scheduling and injected dictionary transport; no native DOM timing claim. */
import assert from 'node:assert/strict';
import { Compartment, EditorState, Transaction, type TransactionSpec } from '@codemirror/state';
import { EditorView, type ViewUpdate } from '@codemirror/view';
import type {
  PromptDocumentKind,
  TagLexicon,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { SpellcheckSession } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-session.ts';
import type { SpellingLookup } from '../../../../../../frontend/sugarprompt/services/src/diagnostics/spelling-contracts.ts';
import { resolveFeatureProfile } from '../../../../../../frontend/sugarprompt/services/src/features/profile.ts';
import { defaultPreferences } from '../../../../../../frontend/sugarprompt/services/src/features/preferences.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { spellingSession } from '../../../../../../frontend/sugarprompt/editor/src/spelling-diagnostics.ts';
import { SpellcheckProvider } from '../../../../../../frontend/sugarprompt/editor/src/spelling-provider.ts';
/** Injected provider methods are the only external DTO boundary; actual service validation remains installed. */
export interface SpellingBoundary {
  check: ((words: readonly string[], signal: AbortSignal) => Promise<SpellingLookup>) | null;
  suggest: ((signal: AbortSignal) => Promise<SpellingLookup>) | null;
  ignore: ((signal: AbortSignal) => Promise<SpellingLookup>) | null;
  add: ((signal: AbortSignal) => Promise<SpellingLookup>) | null;
  available: boolean;
}
/** Shared session remount uses the same actual service, never a copied word-ignore registry. */
export function spellingFixture(
  source = 'helo, tail',
  rich = false,
  kind: PromptDocumentKind = 'prompt',
  shared?: SpellcheckSession,
) {
  const boundary: SpellingBoundary = {
      check: null,
      suggest: null,
      ignore: null,
      add: null,
      available: true,
    },
    counts = { checks: 0, suggestions: 0, ignores: 0, adds: 0 };
  const session =
    shared ??
    new SpellcheckSession({
      identity: Symbol(),
      languageTag: 'en',
      availability: () =>
        boundary.available
          ? { status: 'available', dictionaryAdd: true, sessionIgnore: true }
          : { status: 'unavailable', code: 'backend-unavailable' },
      checkWords: async (words, signal) => {
        counts.checks++;
        return boundary.check
          ? boundary.check(words, signal)
          : { status: 'ready', value: words.map((word) => word !== 'helo') };
      },
      suggest: async (_word, _limit, signal) => {
        counts.suggestions++;
        return boundary.suggest ? boundary.suggest(signal) : { status: 'ready', value: ['hello'] };
      },
      ignoreForSession: async (_word, signal) => {
        counts.ignores++;
        return boundary.ignore ? boundary.ignore(signal) : { status: 'ready', value: true };
      },
      addToDictionary: async (_word, signal) => {
        counts.adds++;
        return boundary.add ? boundary.add(signal) : { status: 'ready', value: true };
      },
    });
  const host = new Compartment(),
    profile = new Compartment();
  let lexicon: TagLexicon | undefined;
  const permission = new Compartment(),
    kindSlot = new Compartment();
  const boundaryView: unknown = Object.create(EditorView.prototype);
  if (!(boundaryView instanceof EditorView)) throw new Error('spelling-fixture-view');
  const initial = {
    permission,
    kindSlot,
    state: createNativeState(source, {
      changeRichRendering: () => {
        throw new Error('Spelling fixture does not own host configuration');
      },
      label: 'Prompt text',
      placeholder: '',
      configuration: [
        richRendering.of(rich),
        host.of(spellingSession.of(session)),
        profile.of(
          featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' })),
        ),
        permission.of(EditorState.readOnly.of(false)),
        kindSlot.of(documentKind.of(kind)),
      ],
      view: () => boundaryView,
      publish: () => {},
      isAlive: () => true,
      lexicon: () => lexicon,
      update: () => {},
      onLayout: undefined,
      onSourceLines: undefined,
      revision: () => 0,
    }),
  };
  let state = initial.state.update({
    selection: { anchor: initial.state.doc.length },
    annotations: Transaction.addToHistory.of(false),
  }).state;
  const originalQueue = globalThis.queueMicrotask,
    nativeTasks: Array<() => void> = [];
  globalThis.queueMicrotask = (callback) => {
    nativeTasks.push(callback);
  };
  let observer: ((transaction: Transaction) => void) | null = null;
  Object.defineProperties(boundaryView, {
    state: { get: () => state },
    composing: { value: false, writable: true },
    compositionStarted: { value: false, writable: true },
    hasFocus: { value: true, writable: true },
    dispatch: {
      value: (input: Transaction | TransactionSpec) => {
        const transaction = input instanceof Transaction ? input : state.update(input);
        assert.equal(transaction.startState, state);
        state = transaction.state;
        const update: unknown = {
          view: boundaryView,
          startState: transaction.startState,
          state,
          transactions: [transaction],
        };
        provider.update(update as ViewUpdate);
        observer?.(transaction);
      },
    },
  });
  const provider = new SpellcheckProvider(boundaryView);
  return {
    view: boundaryView,
    provider,
    session,
    boundary,
    counts,
    initial,
    host,
    profile,
    state: () => state,
    /** A direct real owner operation settles the controlled external promise; queued DOM scheduling is not qualified. */
    check: () => provider.prepare(),
    /** Native source-neutral observer reentry remains an actual transaction boundary. */
    observe(next: ((transaction: Transaction) => void) | null): void {
      observer = next;
    },
    /** Actual catalog input accompanies caller-owned context invalidation. */
    setLexicon(value: TagLexicon | undefined): void {
      lexicon = value;
    },
    /** Exact task bodies can be observed without arbitrary waits or native clock advancement. */
    queued: () => nativeTasks.length,
    /** Mounted teardown releases held scheduler callbacks and restores its isolated global transport. */
    dispose(): void {
      provider.destroy();
      nativeTasks.length = 0;
      globalThis.queueMicrotask = originalQueue;
    },
  };
}
