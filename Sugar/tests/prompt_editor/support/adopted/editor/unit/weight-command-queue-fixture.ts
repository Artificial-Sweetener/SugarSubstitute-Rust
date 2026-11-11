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
/** Controlled native task/measurement transport invokes the genuine weight queue and immutable state owners. */
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import { EditorState, StateEffect, Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import type { FocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff-contracts.ts';
import type { WeightRequest } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { exactWeightDraft } from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
const observed = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Export the unchanged production owner only in memory. */
  load(url, context, next) {
    const result = next(url, context);
    return url === observed
      ? {
          ...result,
          source: readFileSync(new URL(url), 'utf8') + '\nexport { WeightCommands };\n',
        }
      : result;
  },
});
/** Observation exposes command admission and teardown, never CodeMirror internals. */
interface Queue {
  request(
    change: { kind: 'set'; weight: string } | { kind: 'step'; direction: 'decrease' },
    request: WeightRequest,
    handoff: FocusHandoff,
  ): boolean;
  exact(
    change: { kind: 'set'; weight: string },
    request: Extract<WeightRequest, { owner: 'exact' }>,
    handoff: FocusHandoff,
    key: symbol,
  ): boolean;
  update(update: { startState: EditorState; state: EditorState }): void;
  destroy(): void;
}
const module = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts'
)) as unknown as {
  WeightCommands: new (view: EditorView) => Queue;
};

/** Literal source specimens use actual immutable native owners and deterministic task/measure delivery. */
export function control(source: string) {
  let state = weightStateFor(source, source.length),
    restored = 0;
  let admission = true,
    callback: (() => void) | undefined,
    callbackBoundary = 1,
    snapshotReads = 0;
  const viewport: unknown[] = [];
  const anchors: { readonly position: number; readonly offset: number }[] = [];
  const tasks: (() => void)[] = [],
    measures: (() => void)[] = [];
  const originalWindow = globalThis.window;
  globalThis.window = {
    setTimeout(callback: () => void) {
      tasks.push(callback);
      return tasks.length;
    },
    clearTimeout() {},
  } as unknown as Window & typeof globalThis;
  const snapshot = StateEffect.define<{
    readonly position: number;
    readonly offset: number;
  }>({
    /** Native anchor positions map through edits while their measured pixel displacement remains unchanged. */
    map: (value, changes) => ({
      ...value,
      position: changes.mapPos(value.position),
    }),
  });
  const view = {
    get state() {
      return state;
    },
    composing: false,
    compositionStarted: false,
    scrollSnapshot: () => {
      snapshotReads++;
      return snapshot.of({ position: source.length, offset: -13 });
    },
    dispatch(input: readonly Transaction[] | Transaction | TransactionSpec) {
      const transactions: readonly Transaction[] = Array.isArray(input)
        ? input
        : [input instanceof Transaction ? input : state.update(input as TransactionSpec)];
      state = transactions.at(-1)!.state;
      for (const transaction of transactions)
        for (const effect of transaction.effects)
          if (effect.is(snapshot)) anchors.push(effect.value);
      if (callback && --callbackBoundary === 0) {
        const next = callback;
        callback = undefined;
        next();
      }
    },
    requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
      measures.push(() => {
        const read = request.read(view as unknown as EditorView);
        request.write?.(read, view as unknown as EditorView);
      });
    },
  };
  const handoff: FocusHandoff = {
    /** Focus dispatch is source-neutral and observes the final accepted native caret. */
    restore(intent) {
      viewport.push(intent);
      restored++;
      view.dispatch({ annotations: Transaction.addToHistory.of(false) });
      return Promise.resolve(true);
    },
    cancel() {},
  };
  let latest = handoff;
  state = state.update({
    effects: StateEffect.appendConfig.of([
      exactWeightDraft,
      nativeFocusHandoff.of({
        capture: () => handoff,
        current: (candidate) => admission && candidate === latest,
        /** Controlled focus preserves the same exact admitted capability. */
        refocus: (candidate) =>
          admission && candidate === latest
            ? handoff.restore('preserve-viewport')
            : Promise.resolve(false),
      }),
    ]),
  }).state;
  const owner = new module.WeightCommands(view as unknown as EditorView);
  return {
    owner,
    view,
    handoff,
    viewport,
    anchors,
    /** Observe admitted geometry reads without treating counts as evidence of viewport stability. */
    snapshotReads: () => snapshotReads,
    restored: () => restored,
    /** A later repeat supplies a distinct original intent and supersedes the earlier reply. */
    repeatHandoff() {
      latest = { ...handoff };
      return latest;
    },
    /** Independently choose a newer host intent with or without a native state change. */
    supersede() {
      admission = false;
    },
    /** Native baseline adoption installs a fresh real immutable state and retires prior intent. */
    baseline() {
      admission = false;
      state = weightStateFor('baseline');
    },
    /** Deliver a synchronous callback at the next production publication boundary. */
    reenter(next: () => void, boundary = 1) {
      callback = next;
      callbackBoundary = boundary;
    },
    /** Deliver scheduled work without elapsed time or retries. */
    async flush() {
      tasks.shift()?.();
      measures.shift()?.();
      await Promise.resolve();
    },
    /** Deliver the command task while retaining its admitted native measure callback for teardown controls. */
    task() {
      tasks.shift()?.();
    },
    /** Release the finite owner and controlled global transport. */
    dispose() {
      owner.destroy();
      globalThis.window = originalWindow;
    },
  };
}

/** Release the test-only module observation after every consumer has completed. */
export function disposeQueueFixture(): void {
  hook.deregister();
}
