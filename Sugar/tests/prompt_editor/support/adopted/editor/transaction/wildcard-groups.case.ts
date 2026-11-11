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
/** Actual native states prove shared numeric transactions, wrapper reuse and source-bound group retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { prepareWeightChange } from '../../../../../../frontend/sugarprompt/editor/src/weight-command.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { weightSession } from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { WildcardWidget } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-widget.ts';
import { weightStateFor } from './weight-command-state.ts';
import { projectionState } from './projection-state.ts';
import { RegionWidget } from '../../../../../../frontend/sugarprompt/editor/src/region-widget.ts';
import {
  EmphasisWidget,
  ParenthesisWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';

test('CSV structural labels retain literal precedence within their own value while sibling wildcards render', () => {
  const state = projectionState(
    'left,right\n"[SEP|(cat:2) {animal} <lora:m:0.5> \\(x\\)]\n{animal}","{animal}"',
    new Compartment(),
    'wildcard-csv',
  );
  const canonical = prepareNativeStructure(state),
    projection = prepareProjection(state, canonical);
  const widgets: unknown[] = [];
  projection.all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget) widgets.push(value.spec.widget);
  });
  assert.equal(widgets.filter((widget) => widget instanceof RegionWidget).length, 1);
  assert.equal(widgets.filter((widget) => widget instanceof WildcardWidget).length, 4);
  assert.equal(
    widgets.some(
      (widget) =>
        widget instanceof EmphasisWidget ||
        widget instanceof ParenthesisWidget ||
        widget instanceof LoraWidget,
    ),
    false,
  );
  assert.equal(projection.hasLoras, false);
  assert.equal(readExactSource(state), canonical.scopes.values.source);
});

test('wildcard wrapper projection reuses real output across native body selection and leaves implicit group source unchanged', () => {
  const source = '😀\r\n{animal}, {animal}, {csv:animal:Color|word}';
  let state = weightStateFor(source);
  const prepared = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(prepared) }).state;
  assert.equal(readExactSource(state) === source, true);
  assert.equal(prepared.decorations.size, 9);
  const suffixes: WildcardWidget[] = [];
  prepared.all.between(0, state.doc.length, (_from, _to, value) => {
    const widget = value.spec.widget;
    if (widget instanceof WildcardWidget && widget.part === 'suffix') suffixes.push(widget);
  });
  assert.deepEqual(
    suffixes.map((widget) => [
      widget.group.displayTag,
      widget.group.explicit,
      widget.group.numeric,
    ]),
    [
      ['1', false, true],
      ['1', false, true],
      ['word', true, false],
    ],
  );
  state = state.update({ selection: { anchor: 7, head: 5 } }).state;
  assert.equal(state.field(richProjection), prepared);
  assert.equal(undoDepth(state), 0);
});

test('shared keyboard group commands preserve exact source and native undo/redo without decimal arithmetic', () => {
  const source = '😀\r\n{animal}, {animal}';
  let state = weightStateFor(source, 7);
  const changed = prepareWeightChange(state, {
    kind: 'step',
    direction: 'increase',
  });
  assert.equal(changed.length, 2);
  state = changed[1]!.state;
  assert.equal(readExactSource(state) === '😀\r\n{animal|2}, {animal}', true);
  assert.equal(state.field(weightSession)?.kind, 'wildcard');
  assert.equal(undoDepth(state), 1);
  assert.deepEqual(state.field(weightSession)?.range, { from: 3, to: 13 });
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state) === source, true);
  assert.equal(state.selection.main.head, 7);
  redo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state) === '😀\r\n{animal|2}, {animal}', true);
  assert.equal(undoDepth(state), 1);
});

test('wildcard metadata retains literal parenthesis escapes without overlapping hidden suffix decorations', () => {
  const source = '{animal|named\\(tag\\)}, literal\\(text\\)';
  const state = weightStateFor(source);
  const prepared = prepareProjection(state, prepareNativeStructure(state));
  const ranges: [number, number][] = [];
  prepared.all.between(0, state.doc.length, (from, to) => {
    ranges.push([from, to]);
  });
  assert.deepEqual(ranges, [
    [0, 1],
    [1, 7],
    [7, 21],
    [30, 32],
    [36, 38],
  ]);
});

test('spatial group steps map an unrelated directional selection and reject stale or nonnumeric targets', () => {
  const source = '{animal|9}, tail';
  let state = weightStateFor(source, 16, 12);
  const request = {
    owner: 'pointer',
    kind: 'wildcard',
    range: { from: 0, to: 10 },
    freshness: state.field(editorFreshness),
  } as const;
  const changed = prepareWeightChange(state, { kind: 'step', direction: 'increase' }, request);
  assert.equal(changed.length, 2);
  state = changed[1]!.state;
  assert.equal(readExactSource(state) === '{animal|10}, tail', true);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [17, 13]);
  assert.equal(
    prepareWeightChange(state, { kind: 'step', direction: 'increase' }, request).length,
    0,
  );
  for (const other of ['{animal|word}', '{animal}', '{animal|0}', '{animal|01}'])
    assert.equal(
      prepareWeightChange(weightStateFor(other, 2), {
        kind: 'step',
        direction: 'increase',
      }).length,
      0,
    );
});

test('TXT record projections isolate local region ordinals and wildcard groups across CRLF storage boundaries', () => {
  const source = '[SEP|A]\r\n(cat:2)\r\n[SEP|B]\r\n{animal}';
  const state = projectionState(source, new Compartment(), 'wildcard-txt');
  const canonical = prepareNativeStructure(state),
    prepared = prepareProjection(state, canonical);
  const regions: RegionWidget[] = [],
    groups: WildcardWidget[] = [],
    rails: number[][] = [];
  prepared.all.between(0, state.doc.length, (from, to, value) => {
    if (value.spec.widget instanceof RegionWidget) regions.push(value.spec.widget);
    if (value.spec.widget instanceof WildcardWidget) groups.push(value.spec.widget);
    if (value.spec.attributes?.class === 'sp-region-content') rails.push([from, to]);
  });
  assert.deepEqual(
    regions.map((widget) => [widget.target.scopeId, widget.ordinal]),
    [
      ['txt-container:0', 0],
      ['txt-container:2', 0],
    ],
  );
  assert.equal(regions[0]!.target, canonical.regions[0]);
  assert.equal(regions[1]!.target, canonical.regions[1]);
  assert.deepEqual(rails, []);
  assert.equal(groups.length, 2);
  assert.equal(groups[0]!.group, canonical.scopes.scopes[3]!.wildcardGroups[0]);
  assert.equal(groups[1]!.group, canonical.scopes.scopes[3]!.wildcardGroups[0]);
  const bounds: number[][] = [];
  prepared.tokens.between(0, state.doc.length, (from, to) => {
    bounds.push([from, to]);
  });
  assert.deepEqual(bounds, [
    [0, 7],
    [8, 15],
    [16, 23],
    [24, 32],
  ]);
  assert.equal(readExactSource(state), source);
});
