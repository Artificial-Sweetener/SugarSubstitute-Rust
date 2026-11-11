import { editorPresentations } from '../../../EditorPresentationFixture.js';
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
/** Actual controller publication uses immutable CM state/history with only native construction and line-output transport adapted. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { Text, StateEffect } from '@codemirror/state';
import { invertedEffects } from '@codemirror/commands';
import { snapshotPresentation } from '../../../../../../frontend/sugarprompt/editor/src/native-snapshot-policy.ts';
import type { EditorView } from '@codemirror/view';
import type { EditorSnapshot } from '../../../../../../frontend/sugarprompt/editor/src/contracts.ts';
import { NativeSearch } from '../../../../../../frontend/sugarprompt/editor/src/native-search.ts';

const surfaceUrl = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts',
  import.meta.url,
).href;
const tooltipUrl = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/tooltip-parent.ts',
  import.meta.url,
).href;
const historyUrl = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts',
  import.meta.url,
).href;
const linesUrl = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/source-lines.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Keep the complete controller/state/history/projection owners; only DOM construction and prepared line output are inert. */
  load(url, context, next) {
    if (url === tooltipUrl)
      return {
        format: 'module',
        shortCircuit: true,
        source: `
      export class NativeTooltipParent { configuration = []; dispose() {} }
      export function nativeTooltipSpace() { throw new Error("INERT_TOOLTIP_GEOMETRY_UNEXPECTED"); }
    `,
      };
    if (url === surfaceUrl)
      return {
        format: 'module',
        shortCircuit: true,
        source: `
      import { EditorView } from '@codemirror/view';
      import { Transaction } from '@codemirror/state';
      import { historyTransactions } from ${JSON.stringify(historyUrl)};
      import { nativeSourceLines } from ${JSON.stringify(linesUrl)};
      export class NativeSurface {
        static latest;
        constructor(parent, state) {
          this.linePublications = 0;
          const lines = {publish: () => this.linePublications++};
          this.view = {state, compositionStarted: false, composing: false, hasFocus: false,
            plugin: plugin => plugin === nativeSourceLines ? lines : null,
            dispatch(input) {
              const transaction = input instanceof Transaction ? input : this.state.update(input);
              this.update([transaction]);
            },
            update(input) {
              const startState = this.state;
              const transactions = historyTransactions(input);
              this.state = transactions.at(-1).state;
              const update = {view: this, startState, state: this.state, transactions,
                docChanged: transactions.some(item => item.docChanged),
                selectionSet: transactions.some(item => !!item.selection), focusChanged: false};
              for (const listener of this.state.facet(EditorView.updateListener)) listener(update);
            }
          };
          this.view.dispatch = this.view.dispatch.bind(this.view);
          NativeSurface.latest = this;
        }
        setState(state) { this.view.state = state; return state; }
        retireHandoff() {}
        admitSource() { return true; }
        currentHandoff() { return true; }
        handoff() { return {restore: () => Promise.resolve(false), cancel() {}}; }
        focus() { return Promise.resolve(false); }
        dispose() {}
      }
    `,
      };
    return next(url, context);
  },
});
const { mountEditor } = await import(
  '../../../../../../frontend/sugarprompt/editor/src/controller.ts'
);
const surfaceModule = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts'
)) as unknown as {
  NativeSurface: { latest: { view: EditorView; linePublications: number } };
};
let sourceReads = 0,
  searchObservations = 0;
const originalToString = Text.prototype.toString,
  originalObserve = NativeSearch.prototype.observe;
Text.prototype.toString = function (this: Text): string {
  sourceReads++;
  return originalToString.call(this);
};
NativeSearch.prototype.observe = function (this: NativeSearch): void {
  searchObservations++;
  return originalObserve.call(this);
};
const visual = StateEffect.define<number>();

/** Create one actual controller lifetime and preserve the complete initial publication before test callbacks are armed. */
function fixture(observe: (snapshot: EditorSnapshot) => void = () => {}) {
  const seen: EditorSnapshot[] = [];
  const api = mountEditor({} as HTMLElement, {
    ...editorPresentations,
    source: 'a\r\n😀b',
    label: 'Test',
    placeholder: '',
    readOnly: false,
    onUpdate(snapshot) {
      seen.push(snapshot);
      observe(snapshot);
    },
  });
  return { api, seen, surface: surfaceModule.NativeSurface.latest };
}

test('unchanged visual transactions avoid actual source materialization while search and lines still observe', () => {
  const f = fixture();
  try {
    assert.equal(f.seen.length, 1);
    const reads = sourceReads,
      searches = searchObservations,
      lines = f.surface.linePublications;
    for (let i = 0; i < 20; i++)
      f.surface.view.dispatch({
        effects: visual.of(i),
        annotations: snapshotPresentation.of(true),
      });
    assert.equal(sourceReads - reads, 0);
    assert.equal(f.seen.length, 1);
    assert.equal(searchObservations - searches, 20);
    assert.equal(f.surface.linePublications - lines, 20);
  } finally {
    f.api.destroy();
  }
});

test('ordinary equal observations and mixed transaction bundles preserve native host notifications', () => {
  const f = fixture();
  try {
    const reads = sourceReads;
    f.surface.view.dispatch({ effects: visual.of(1) });
    assert.equal(f.seen.length, 2);
    assert.equal(sourceReads - reads, 1);
    const first = f.surface.view.state.update({
      effects: visual.of(2),
      annotations: snapshotPresentation.of(true),
    });
    const second = first.state.update({ effects: visual.of(3) });
    f.surface.view.update([first, second]);
    assert.equal(f.seen.length, 3);
    const count = sourceReads;
    const next = f.surface.view.state.update({
      effects: visual.of(4),
      annotations: snapshotPresentation.of(true),
    });
    f.surface.view.update([
      next,
      next.state.update({
        effects: visual.of(5),
        annotations: snapshotPresentation.of(true),
      }),
    ]);
    assert.equal(f.seen.length, 3);
    assert.equal(sourceReads - count, 0);
  } finally {
    f.api.destroy();
  }
});

test('public conditioning context changes preserve equal snapshot notification for inspection consumers', () => {
  const f = fixture();
  try {
    const before = f.api.snapshot(),
      count = f.seen.length;
    f.api.setConditioningContext({
      key: 'closed-context',
      mode: 'independent',
    });
    assert.ok(f.seen.length > count);
    const after = f.api.snapshot();
    assert.equal(
      (Object.keys(before) as (keyof EditorSnapshot)[]).every((key) => before[key] === after[key]),
      true,
    );
    const inspected = f.api.inspectConditioning();
    assert.equal(inspected.status, 'ready');
    if (inspected.status === 'ready')
      assert.equal(inspected.context.key === 'closed-context', true);
  } finally {
    f.api.destroy();
  }
});

test('annotation never suppresses changed source or native history-only observations', () => {
  const f = fixture();
  const historyPulse = StateEffect.define<number>();
  try {
    f.surface.view.dispatch({
      effects: StateEffect.appendConfig.of(
        invertedEffects.of((transaction) =>
          transaction.effects.flatMap((effect) =>
            effect.is(historyPulse) ? [historyPulse.of(-effect.value)] : [],
          ),
        ),
      ),
    });
    const document = f.surface.view.state.doc,
      before = f.api.snapshot(),
      count = f.seen.length;
    f.surface.view.dispatch({
      effects: historyPulse.of(1),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.surface.view.state.doc === document, true);
    assert.equal(f.seen.length, count + 1);
    assert.equal(f.seen.at(-1)!.undoDepth, before.undoDepth + 1);
    const changed = f.seen.length;
    f.surface.view.dispatch({
      changes: { from: 0, insert: 'X' },
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, changed + 1);
    assert.equal(f.seen.at(-1)!.source === 'Xa\r\n😀b', true);
  } finally {
    f.api.destroy();
  }
});

test('a fresh public query cannot consume the next changed notification', () => {
  const f = fixture();
  try {
    Object.assign(f.surface.view, { hasFocus: true });
    const reads = sourceReads,
      queried = f.api.snapshot();
    assert.equal(queried.focused, true);
    assert.equal(sourceReads - reads, 1);
    f.surface.view.dispatch({
      effects: visual.of(1),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, 2);
    assert.equal(f.seen[1]!.focused, true);
    assert.equal(f.seen[1] !== queried, true);
    assert.equal(sourceReads - reads, 2);
    f.surface.view.dispatch({
      effects: visual.of(2),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, 2);
  } finally {
    f.api.destroy();
  }
});

test('host mutation of a delivered snapshot cannot poison private eligibility or fresh queries', () => {
  const f = fixture();
  try {
    Reflect.set(f.seen[0]!, 'source', 'foreign');
    Reflect.set(f.seen[0]!, 'focused', true);
    const reads = sourceReads;
    f.surface.view.dispatch({
      effects: visual.of(1),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, 1);
    assert.equal(sourceReads - reads, 0);
    const fresh = f.api.snapshot();
    assert.equal(fresh.source === 'a\r\n😀b', true);
    assert.equal(fresh.focused, false);
  } finally {
    f.api.destroy();
  }
});

test('nested changed publication stamps newer output before the older callback returns', () => {
  let armed = false,
    enter = () => {};
  const f = fixture(() => {
    if (armed) {
      armed = false;
      enter();
    }
  });
  try {
    enter = () => f.surface.view.dispatch({ changes: { from: 0, insert: 'X' } });
    armed = true;
    Object.assign(f.surface.view, { hasFocus: true });
    f.surface.view.dispatch({
      effects: visual.of(1),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, 3);
    assert.equal(f.seen[2]!.source === 'Xa\r\n😀b', true);
    const reads = sourceReads;
    f.surface.view.dispatch({
      effects: visual.of(2),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, 3);
    assert.equal(sourceReads - reads, 0);
  } finally {
    f.api.destroy();
  }
});

for (const raw of [null, undefined])
  test(`raw ${raw === null ? 'null' : 'undefined'} callback failure leaves the same undelivered observation eligible`, () => {
    let armed = false;
    const f = fixture(() => {
      if (armed) {
        armed = false;
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw raw;
      }
    });
    try {
      armed = true;
      Object.assign(f.surface.view, { hasFocus: true });
      let caught = false;
      try {
        f.surface.view.dispatch({
          effects: visual.of(1),
          annotations: snapshotPresentation.of(true),
        });
      } catch (cause) {
        caught = true;
        assert.equal(cause, raw);
      }
      assert.equal(caught, true);
      f.surface.view.dispatch({
        effects: visual.of(2),
        annotations: snapshotPresentation.of(true),
      });
      assert.equal(f.seen.length, 3);
      f.surface.view.dispatch({
        effects: visual.of(3),
        annotations: snapshotPresentation.of(true),
      });
      assert.equal(f.seen.length, 3);
    } finally {
      f.api.destroy();
    }
  });

test('an older throwing callback never rewinds a newer successful delivery', () => {
  const raw = Object.freeze({ code: 'CONTROLLED_PUBLICATION_FAILURE' });
  let armed = false,
    enter = () => {};
  const f = fixture(() => {
    if (armed) {
      armed = false;
      enter();
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw raw;
    }
  });
  try {
    enter = () => f.surface.view.dispatch({ changes: { from: 0, insert: 'X' } });
    armed = true;
    Object.assign(f.surface.view, { hasFocus: true });
    assert.throws(
      () =>
        f.surface.view.dispatch({
          effects: visual.of(1),
          annotations: snapshotPresentation.of(true),
        }),
      (cause) => cause === raw,
    );
    assert.equal(f.seen.length, 3);
    const reads = sourceReads;
    f.surface.view.dispatch({
      effects: visual.of(2),
      annotations: snapshotPresentation.of(true),
    });
    assert.equal(f.seen.length, 3);
    assert.equal(sourceReads - reads, 0);
  } finally {
    f.api.destroy();
  }
});

test('disposal during delivery retires eligibility before old native callbacks resume', () => {
  let armed = false,
    retire = () => {};
  const f = fixture(() => {
    if (armed) retire();
  });
  retire = () => f.api.destroy();
  armed = true;
  Object.assign(f.surface.view, { hasFocus: true });
  f.surface.view.dispatch({
    effects: visual.of(1),
    annotations: snapshotPresentation.of(true),
  });
  const reads = sourceReads,
    publications = f.seen.length;
  f.surface.view.dispatch({
    effects: visual.of(2),
    annotations: snapshotPresentation.of(true),
  });
  assert.equal(f.seen.length, publications);
  assert.equal(sourceReads - reads, 0);
  assert.throws(() => f.api.snapshot(), /Editor disposed/u);
});

test.after(() => {
  Text.prototype.toString = originalToString;
  NativeSearch.prototype.observe = originalObserve;
  hook.deregister();
});
