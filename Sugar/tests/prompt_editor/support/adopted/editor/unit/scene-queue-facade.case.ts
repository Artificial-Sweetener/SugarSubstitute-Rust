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
/** Public facade admission runs unchanged controller/configuration/service bodies with only an inert native-view transport. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { undoDepth, redoDepth } from '@codemirror/commands';
import type { NativeSurface } from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import { prepareSceneQueue } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  sceneQueueHost,
  type SceneQueueHost,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-queue.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';

const surfaceUrl = new URL(
    '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts',
    import.meta.url,
  ).href,
  tooltipUrl = new URL(
    '../../../../../../frontend/sugarprompt/editor/src/tooltip-parent.ts',
    import.meta.url,
  ).href;
const historyUrl = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Replace platform construction only; supported immutable state, history and public update listeners execute normally. */
  load(url, context, next) {
    if (url === tooltipUrl)
      return {
        format: 'module',
        shortCircuit: true,
        source: `export class NativeTooltipParent { configuration = []; dispose() {} }
          /** This inert view has no native geometry; any placement read must fail this owner proof. */
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
        export class NativeSurface {
          static latest;
          constructor(parent, state) {
            this.retirements = 0;
            this.view = { state, composing: false, compositionStarted: false, hasFocus: false, plugin: () => null,
              dispatch(input) {
                const startState = this.state;
                const transaction = input instanceof Transaction ? input : startState.update(input);
                const transactions = historyTransactions([transaction]);
                this.state = transactions.at(-1).state;
                const update = { view: this, startState, state: this.state, transactions,
                  docChanged: transaction.docChanged, selectionSet: !!transaction.selection, focusChanged: false };
                for (const listener of this.state.facet(EditorView.updateListener)) listener(update);
              }
            };
            // Native commands receive EditorView's bound public dispatch method.
            this.view.dispatch = this.view.dispatch.bind(this.view);
            NativeSurface.latest = this;
          }
          setState(state) { this.view.state = state; return state; }
          retireHandoff() { this.retirements++; }
          admitSource() { this.retireHandoff(); return true; }
          handoff() { return { restore: () => Promise.resolve(false), cancel() {} }; }
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
  NativeSurface: {
    latest: Pick<NativeSurface, 'view'> & { retirements: number };
  };
};

/** Genuine owner output supplies canonical current and deliberately stale queue capabilities. */
function queue(sourceRevision: number): SceneQueueHost {
  return {
    capability: prepareSceneQueue({
      revision: 'queue',
      sourceRevision,
      queueableKeys: ['scene'],
      queue: async () => ({ status: 'queued' }),
    }),
    observe() {},
    report() {},
  };
}

for (const ordering of ['current', 'stale', 'kind', 'startup', 'preedit', 'destroy'] as const)
  test(`public queue admission rejects malformed and foreign envelopes before ${ordering} eligibility or cleanup`, () => {
    let publications = 0;
    const api = mountEditor({} as HTMLElement, {
      ...editorPresentations,
      source: 'cat',
      label: 'Test',
      placeholder: '',
      readOnly: false,
      onUpdate() {
        publications++;
      },
    });
    const surface = surfaceModule.NativeSurface.latest;
    try {
      api.updateHost({ mode: 'exact-edit', source: 'first' });
      api.updateHost({ mode: 'exact-edit', source: 'second' });
      api.undo();
      assert.deepEqual([api.snapshot().undoDepth, api.snapshot().redoDepth], [1, 1]);
      const installed = queue(api.snapshot().revision);
      assert.equal(api.setSceneQueueHost(installed), true);
      assert.strictEqual(surface.view.state.facet(sceneQueueHost), installed);
      if (ordering === 'stale') api.replaceSource(0, 5, 'third');
      if (ordering === 'kind') api.setDocumentKind('wildcard-txt');
      if (ordering === 'startup') Object.assign(surface.view, { compositionStarted: true });
      if (ordering === 'preedit')
        Object.assign(surface.view, {
          compositionStarted: true,
          composing: true,
        });
      if (ordering === 'destroy') api.destroy();
      const state = surface.view.state,
        freshness = state.field(editorFreshness),
        before = { publications, retirements: surface.retirements };
      const authentic = queue(installed.capability.sourceRevision);
      const history = [undoDepth(state), redoDepth(state)];
      for (const malformed of [
        false,
        undefined,
        {},
        { capability: { sourceRevision: -1 } },
        {
          ...authentic,
          capability: { ...authentic.capability, sourceRevision: -1 },
        },
        { ...authentic, capability: { ...authentic.capability } },
        { ...authentic, observe: null },
        { ...authentic, report: false },
      ]) {
        assert.throws(() => api.setSceneQueueHost(malformed as SceneQueueHost), TypeError);
        assert.strictEqual(surface.view.state, state);
        assert.strictEqual(surface.view.state.field(editorFreshness), freshness);
        assert.deepEqual({ publications, retirements: surface.retirements }, before);
        assert.deepEqual([undoDepth(surface.view.state), redoDepth(surface.view.state)], history);
      }
      if (ordering === 'destroy')
        assert.throws(() => api.setSceneQueueHost(authentic), /Editor disposed/u);
      else {
        const eligible = ordering === 'current';
        assert.equal(api.setSceneQueueHost(authentic), eligible);
        assert.strictEqual(surface.view.state.facet(sceneQueueHost), eligible ? authentic : null);
      }
    } finally {
      api.destroy();
    }
  });

test('public genuine stale queue cleanup remains deliberate and typed; current replacement preserves prepared identity', () => {
  const api = mountEditor({} as HTMLElement, {
    ...editorPresentations,
    source: 'cat',
    label: 'Test',
    placeholder: '',
    readOnly: false,
    onUpdate() {},
  });
  const surface = surfaceModule.NativeSurface.latest;
  try {
    const installed = queue(0);
    assert.equal(api.setSceneQueueHost(installed), true);
    api.replaceSource(0, 3, 'dog');
    assert.equal(api.setSceneQueueHost(installed), false);
    assert.strictEqual(surface.view.state.facet(sceneQueueHost), null);
    const current = queue(api.snapshot().revision);
    assert.equal(api.setSceneQueueHost(current), true);
    assert.strictEqual(surface.view.state.facet(sceneQueueHost), current);
  } finally {
    api.destroy();
  }
});

test.after(() => hook.deregister());
