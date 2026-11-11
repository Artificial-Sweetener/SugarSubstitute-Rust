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
/** A finite nominal view observes actual native history, source and physical settlement without browser timing claims. */
import assert from 'node:assert/strict';
import { mock } from 'node:test';
import {
  Compartment,
  EditorState,
  StateEffect,
  Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { projectionState } from './projection-state.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { nativeConditioning } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-state.ts';
import { documentDiagnostics } from '../../../../../../frontend/sugarprompt/editor/src/document-diagnostics.ts';
import { SemanticRefresh } from '../../../../../../frontend/sugarprompt/editor/src/semantic-refresh.ts';
import {
  GraphemeLineCache,
  nativeGraphemes,
} from '../../../../../../frontend/sugarprompt/editor/src/native-graphemes.ts';
import {
  nativeInputSettlement,
  type NativeInputSettlement,
} from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';

/** The nominal view applies exact actual transactions and owner callbacks; its finite job queue withholds native measurement. */
export function deletionFixture(source: string) {
  const tasks: (() => void)[] = [],
    background: (() => void)[] = [],
    jobs: (() => void)[] = [];
  const backgroundScheduler = mock.method(globalThis, 'setTimeout', (callback: () => void) => {
    background.push(callback);
    return 0;
  });
  const scheduler = mock.method(globalThis, 'queueMicrotask', (callback: () => void) =>
    tasks.push(callback),
  );
  const rendering = new Compartment(),
    permission = new Compartment(),
    profile = new Compartment();
  let held = false,
    composing = false;
  let observer: (() => void) | null = null,
    observations = 0;
  const input: NativeInputSettlement = {
    /** Controlled observations exercise publication fences rather than browser timing. */
    hasHeldKey() {
      if (observations-- <= 0) {
        const callback = observer;
        observer = null;
        callback?.();
      }
      return held;
    },
    /** Composition is observed through the genuine settlement facet. */
    hasCompositionInput: () => composing,
    /** No composition resource exists at this nominal boundary. */
    cancelCompositionInput() {},
    /** This owner never reserves a command. */
    reserveCommand() {
      throw new Error('unexpected-command-reservation');
    },
  };
  let state = projectionState(source, rendering);
  state = state.update({
    effects: StateEffect.appendConfig.of([
      nativeConditioning,
      documentDiagnostics,
      permission.of(EditorState.readOnly.of(false)),
      profile.of([]),
      nativeInputSettlement.of(input),
    ]),
  }).state;
  const semantic = prepareNativeStructure(state),
    projection = prepareProjection(state, semantic);
  state = state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installProjection.of(projection),
    ],
    annotations: Transaction.addToHistory.of(false),
  }).state;
  const cache = new GraphemeLineCache();
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('nominal-view-missing');
  const view = boundary;
  Object.defineProperties(view, {
    state: { get: () => state },
    compositionStarted: { get: () => composing },
    plugin: {
      value: (plugin: unknown) => (plugin === nativeGraphemes ? { cache } : null),
    },
    dispatch: {
      value: (spec: TransactionSpec | Transaction) => {
        const transaction = spec instanceof Transaction ? spec : state.update(spec),
          startState = state;
        assert.equal(transaction.startState, startState);
        state = transaction.state;
        owner?.update({ startState, transactions: [transaction] });
      },
    },
    requestMeasure: {
      value: (request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) => {
        jobs.push(() => {
          const measured = request.read(view);
          request.write?.(measured, view);
        });
      },
    },
  });
  const owner = new SemanticRefresh(view);
  return {
    view,
    rendering,
    permission,
    profile,
    /** Observe the one actual current editor state. */
    state: () => state,
    /** Execute finite off-input callbacks without measurement or clock movement. */
    drain() {
      let count = 0;
      while (tasks.length || background.length) {
        assert.ok(++count < 20);
        (tasks.shift() ?? background.shift())!();
      }
    },
    /** Execute recorded supported native measurement callbacks. */
    measure() {
      while (jobs.length) jobs.shift()!();
    },
    /** Composition/held key observations cannot publish source-input output. */
    block(kind: 'held' | 'composition') {
      if (kind === 'held') held = true;
      else composing = true;
    },
    /** Follow actual keyup/blur/pointer settlement after the held physical key retires. */
    release(signal: 'key' | 'blur' | 'pointer') {
      held = false;
      if (signal === 'pointer') owner.pointerStart();
      else owner.keyEnd();
    },
    /** Composition start explicitly cancels captured history work. */
    compositionStart() {
      composing = true;
      owner.compositionStart();
    },
    /** Challenge an actual currentness observation with synchronous real dispatch. */
    reenter(callback: () => void, afterObservations = 0) {
      observer = callback;
      observations = afterObservations;
    },
    /** Cancel all owned callbacks and restore the sole scheduling boundary. */
    destroy() {
      owner!.destroy();
      scheduler.mock.restore();
      backgroundScheduler.mock.restore();
    },
  };
}
