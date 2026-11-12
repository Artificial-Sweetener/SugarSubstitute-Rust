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
/** Regional rails rotate from the host accent, fall back to the pinned base, and re-prepare when the accent changes. */
import { editorPresentations } from '../../../EditorPresentationFixture.js';
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect, type EditorState } from '@codemirror/state';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  DEFAULT_REGION_PALETTE,
  regionPalette,
} from '../../../../../../frontend/sugarprompt/editor/src/region-paint.ts';
import { prepareRegionColor } from '../../../../../../frontend/sugarprompt/editor/src/region-color.ts';
import { NativeConfiguration } from '../../../../../../frontend/sugarprompt/editor/src/native-configuration.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { parseRegionHex } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { projectionState } from '../transaction/projection-state.ts';

/** Collect the rail colours a projection paints, in source order. */
function railColors(state: EditorState): string[] {
  const colors: string[] = [];
  state.field(richProjection).decorations.between(0, state.doc.length, (_from, _to, value) => {
    const style = (value.spec as { attributes?: { style?: string } }).attributes?.style;
    const color = style ? /--sp-region-color:([^;]+)/.exec(style)?.[1] : undefined;
    if (color && !colors.includes(color)) colors.push(color);
  });
  return colors;
}

/** Prepare and install the projection of the current state. */
function project(state: EditorState): EditorState {
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

test('rails rotate from the configured accent and fall back to the pinned base', () => {
  const palette = new Compartment();
  const source = 'global\n[SEP|sky]\nblue\n[SEP]\nground';
  let state = initializeNativeStructure(projectionState(source));
  state = state.update({ effects: StateEffect.appendConfig.of(palette.of([])) }).state;
  state = project(state);
  const pinned = [prepareRegionColor(0, 2).normal, prepareRegionColor(1, 2).normal];
  assert.deepEqual(railColors(state), pinned);
  assert.equal(pinned[0], DEFAULT_REGION_PALETTE);
  state = state.update({ effects: palette.reconfigure(regionPalette.of('#cc3355')) }).state;
  assert.notEqual(state.field(richProjection).sourceIdentity, state.field(editorFreshness).source);
  state = project(state);
  const base = parseRegionHex('#cc3355');
  assert.ok(base);
  assert.deepEqual(railColors(state), [
    prepareRegionColor(0, 2, base).normal,
    prepareRegionColor(1, 2, base).normal,
  ]);
  assert.equal(railColors(state)[0], '#cc3355');
});

test('the host accent is normalized, changes only when its colour does, and rejects malformed values', () => {
  const configuration = new NativeConfiguration({
    ...editorPresentations,
    source: '',
    label: 'Test',
    placeholder: '',
    readOnly: false,
    onUpdate() {},
    regionAccent: '#CC3355',
  });
  assert.equal(configuration.values.regionAccent, '#cc3355');
  assert.deepEqual(configuration.prepare({ regionAccent: '#cc3355' }, 0).changedKeys, []);
  assert.deepEqual(configuration.prepare({ regionAccent: null }, 0).changedKeys, ['regionAccent']);
  assert.throws(() => configuration.prepare({ regionAccent: 'red' }, 0), TypeError);
});
