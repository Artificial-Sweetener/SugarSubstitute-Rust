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
/** Pointer separator hover reaches the host observer while host-linked hover stays silent and yields to the pointer. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, type EditorState, type TransactionSpec } from '@codemirror/state';
import {
  pointRegionSeparator,
  regionHover,
  regionHoverObserver,
  showLinkedRegionHover,
} from '../../../../../../frontend/sugarprompt/editor/src/region-hover.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { projectionState } from '../transaction/projection-state.ts';

/** Mount a projected prompt with an observed hover field behind a minimal native view boundary. */
function fixture(source: string) {
  const reported: (number | null)[] = [];
  const initial = initializeNativeStructure(projectionState(source));
  let state: EditorState = initial.update({
    effects: [
      installProjection.of(prepareProjection(initial, prepareNativeStructure(initial))),
      StateEffect.appendConfig.of([
        regionHover,
        regionHoverObserver.of((ordinal) => reported.push(ordinal)),
      ]),
    ],
  }).state;
  const view = {
    get state() {
      return state;
    },
    dispatch(spec: TransactionSpec) {
      state = state.update(spec).state;
    },
  } as Parameters<typeof showLinkedRegionHover>[0];
  const scope = Object.keys(state.field(richProjection).regionHovers)[0] ?? '';
  const row = (ordinal: number) => ({
    dataset: { regionIndex: String(ordinal), regionScope: scope },
  });
  const hovered = () => view.state.field(regionHover)?.target.ordinal ?? null;
  return { view, reported, row, hovered };
}

test('pointer hover of a prompt separator is reported by ordinal and its end by null', () => {
  const { view, reported, row, hovered } = fixture('global\n[SEP|sky]\nblue\n[SEP]\nground');
  pointRegionSeparator(view, row(1));
  pointRegionSeparator(view, row(1));
  pointRegionSeparator(view, row(0));
  assert.equal(hovered(), 0);
  pointRegionSeparator(view, null);
  assert.equal(hovered(), null);
  assert.deepEqual(reported, [1, 0, null]);
});

test('host-linked hover shows a separator silently and never overrides the pointer', () => {
  const { view, reported, row, hovered } = fixture('global\n[SEP|sky]\nblue\n[SEP]\nground');
  assert.equal(showLinkedRegionHover(view, 1), true);
  assert.equal(hovered(), 1);
  assert.equal(showLinkedRegionHover(view, 5), false);
  assert.equal(hovered(), null);
  assert.equal(showLinkedRegionHover(view, 0), true);
  pointRegionSeparator(view, null);
  assert.equal(hovered(), null);
  assert.deepEqual(reported, []);
  pointRegionSeparator(view, row(1));
  assert.equal(showLinkedRegionHover(view, 0), false);
  assert.equal(showLinkedRegionHover(view, null), false);
  assert.equal(hovered(), 1);
  assert.equal(showLinkedRegionHover(view, 1), true);
  assert.deepEqual(reported, [1]);
  assert.throws(() => showLinkedRegionHover(view, -1), TypeError);
  assert.throws(() => showLinkedRegionHover(view, 0.5), TypeError);
});

test('a source edit retires linked hover', () => {
  const { view, hovered } = fixture('global\n[SEP|sky]\nblue');
  assert.equal(showLinkedRegionHover(view, 0), true);
  view.dispatch({ changes: { from: 0, insert: 'x' } });
  assert.equal(hovered(), null);
});
