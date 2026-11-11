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
/** Canonical wildcard shell descriptors retain catalog paint metadata through bounded feedback without native source/history mutation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { prepareWildcardCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { wildcardCatalog } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-catalog.ts';
import { WildcardWidget } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-widget.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { projectWeightFeedback } from '../../../../../../frontend/sugarprompt/editor/src/projection-feedback.ts';
import {
  showWheelFeedback,
  wheelFeedback,
} from '../../../../../../frontend/sugarprompt/editor/src/wheel-feedback.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { projectionState } from './projection-state.ts';

/** Observe actual canonical descriptors without reading DOM, resolving catalogs or reproducing paint decisions. */
function shells(projection: ReturnType<typeof prepareProjection>): readonly WildcardWidget[] {
  const result: WildcardWidget[] = [];
  projection.all.between(0, 100, (_from, _to, value) => {
    if (value.spec.widget instanceof WildcardWidget) result.push(value.spec.widget);
  });
  return result;
}

test('catalog publication gives both shell halves exact prepared found/missing/pending/unavailable status and invalidates unequal paint', () => {
  for (const status of ['ready', 'pending', 'unavailable'] as const) {
    const catalog = prepareWildcardCatalog(
      status === 'ready'
        ? {
            status,
            revision: status,
            textIdentifiers: ['animals'],
            csvSources: [],
          }
        : { status, revision: status },
    );
    const state = projectionState('{animals|2}, {missing|3}').update({
      effects: StateEffect.appendConfig.of(wildcardCatalog.of(catalog)),
    }).state;
    const widgets = shells(prepareProjection(state, prepareNativeStructure(state)));
    assert.deepEqual(
      widgets.map((widget) => widget.resolution),
      status === 'ready'
        ? ['found', 'found', 'missing', 'missing']
        : [status, status, status, status],
    );
    const first = widgets[0]!;
    assert.equal(
      first.eq(new WildcardWidget(first.token, first.part, first.group, first.resolution)),
      true,
    );
    assert.equal(
      first.eq(
        new WildcardWidget(
          first.token,
          first.part,
          first.group,
          first.resolution === 'missing' ? 'found' : 'missing',
        ),
      ),
      false,
    );
    assert.equal(state.doc.toString(), '{animals|2}, {missing|3}');
    assert.equal(undoDepth(state), 0);
  }
});

test('feedback replacement and retirement retain missing status, literal group identity and unaffected prepared shells', () => {
  const catalog = prepareWildcardCatalog({
    status: 'ready',
    revision: 'ready',
    textIdentifiers: ['animals'],
    csvSources: [],
  });
  let state = projectionState('{animals|2}, {missing|3}').update({
    effects: StateEffect.appendConfig.of([wildcardCatalog.of(catalog), wheelFeedback]),
  }).state;
  const prepared = prepareProjection(state, prepareNativeStructure(state)),
    before = shells(prepared);
  state = state.update({
    effects: showWheelFeedback.of({
      kind: 'wildcard',
      range: { from: 13, to: 24 },
      freshness: state.field(editorFreshness),
    }),
  }).state;
  const feedback = projectWeightFeedback(prepared, state),
    active = shells(feedback);
  assert.deepEqual(
    active.map((widget) => [widget.resolution, widget.adjustment]),
    [
      ['found', 'none'],
      ['found', 'none'],
      ['missing', 'wheel'],
      ['missing', 'wheel'],
    ],
  );
  assert.equal(active[0], before[0]);
  assert.equal(active[1], before[1]);
  assert.equal(active[2]!.group, before[2]!.group);
  state = state.update({ effects: showWheelFeedback.of(null) }).state;
  assert.deepEqual(
    shells(projectWeightFeedback(feedback, state)).map((widget) => [
      widget.resolution,
      widget.adjustment,
    ]),
    [
      ['found', 'none'],
      ['found', 'none'],
      ['missing', 'none'],
      ['missing', 'none'],
    ],
  );
  assert.equal(state.doc.toString(), '{animals|2}, {missing|3}');
  assert.equal(undoDepth(state), 0);
});
