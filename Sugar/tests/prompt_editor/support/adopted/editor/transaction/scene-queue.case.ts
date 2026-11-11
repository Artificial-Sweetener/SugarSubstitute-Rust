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
/** Actual native state proves prepared queue lookup reuse, exact coordinates and source-bound host retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect } from '@codemirror/state';
import { undoDepth, undo } from '@codemirror/commands';
import { prepareSceneQueue } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  sceneQueueHost,
  sceneQueuePublication,
  sceneQueueTargets,
  prepareSceneQueueTargets,
  installSceneQueueTargets,
  sceneQueueTargetAt,
  currentSceneQueueTarget,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-queue.ts';
import {
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { projectionState } from './projection-state.ts';

/** Native parsing remains the existing semantic owner; this fixture publishes only detached host keys. */
function prepared() {
  const configuration = new Compartment(),
    permission = new Compartment();
  const host = {
    capability: prepareSceneQueue({
      revision: 'workflow',
      sourceRevision: 0,
      queueableKeys: ['strasse', 'cafe'],
      queue: async () => ({ status: 'queued' as const }),
    }),
    observe: () => {},
    report: () => {},
  };
  let state = projectionState('🙂 shared\r\n**Straße\rfirst\n**Cafe\r\nlast');
  state = state.update({
    effects: StateEffect.appendConfig.of([
      sceneQueuePublication,
      sceneQueueTargets,
      configuration.of(sceneQueueHost.of(host)),
      permission.of([]),
    ]),
  }).state;
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
  const targets = prepareSceneQueueTargets(state)!;
  state = state.update({ effects: installSceneQueueTargets.of(targets) }).state;
  return { state, targets, configuration, permission, host };
}

test('bulk-converted mixed newline scene lookup reuses actual requests across directional selection without source/history changes', () => {
  const fixture = prepared(),
    { targets, host } = fixture;
  let { state } = fixture;
  assert.equal(targets.at(0), null);
  assert.equal(
    targets.at(state.doc.toString().indexOf('**Straße')),
    host.capability.request('strasse'),
  );
  const target = sceneQueueTargetAt(state, state.doc.length)!;
  assert.equal(target.request, host.capability.request('cafe'));
  const source = readExactSource(state);
  state = state.update({
    selection: { anchor: state.doc.length, head: 1 },
  }).state;
  assert.equal(state.field(sceneQueueTargets), targets);
  assert.equal(currentSceneQueueTarget(state, target), true);
  assert.equal(undoDepth(state), 0);
  assert.equal(readExactSource(state), source);
  for (const position of [-1, state.doc.length + 1, 0.5])
    assert.throws(() => targets.at(position), RangeError);
});

test('source edits, native undo, command context and host replacement reject old targets and stale prepared lookup', () => {
  for (const change of ['source', 'context', 'host'] as const) {
    const fixture = prepared(),
      { targets, configuration, host } = fixture;
    let { state } = fixture;
    const target = sceneQueueTargetAt(state, state.doc.length)!;
    state = state.update(
      change === 'source'
        ? exactReplacement(state, 0, 0, 'x')
        : {
            effects:
              change === 'context'
                ? invalidateCommandContext.of(null)
                : configuration.reconfigure(sceneQueueHost.of({ ...host })),
          },
    ).state;
    assert.equal(currentSceneQueueTarget(state, target), false);
    if (change === 'source') {
      state = state.update({
        effects: installSceneQueueTargets.of(targets),
      }).state;
      assert.equal(state.field(sceneQueuePublication), null);
      assert.equal(state.field(sceneQueueTargets), null);
      assert.equal(
        undo({
          state,
          dispatch: (transaction) => {
            state = transaction.state;
          },
        }),
        true,
      );
      assert.equal(state.field(sceneQueuePublication), null);
    }
  }
});
