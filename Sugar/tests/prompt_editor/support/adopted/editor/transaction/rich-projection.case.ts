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
/** Real native state proves prepared emphasis ownership, exact ranges and publication freshness. */
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, Transaction, StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { projectionState, projectionRanges } from './projection-state.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { RegionWidget } from '../../../../../../frontend/sugarprompt/editor/src/region-widget.ts';
import {
  SourceWidget,
  EmphasisWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { WildcardWidget } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-widget.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

test('nested exact-source emphasis compiles only wrappers across Unicode and CRLF', () => {
  let state = projectionState('🙂\r\n((cat:1.20):2.00)');
  const projection = prepareProjection(state, prepareNativeStructure(state));
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({
    effects: installProjection.of(projection),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  assert.deepEqual(projectionRanges(state), [
    [3, 4],
    [4, 5],
    [8, 14],
    [14, 20],
  ]);
  assert.equal(state.field(richProjection), projection);
  assert.equal(undoDepth(state), 0);
  state = state.update({ selection: { anchor: 8, head: 5 } }).state;
  assert.equal(state.field(richProjection), projection);
  assert.equal(state.selection.main.anchor, 8);
  assert.equal(state.selection.main.head, 5);
});

test('changed source rejects old publication and raw mode removes derived wrappers without history', () => {
  const presentation = new Compartment();
  let state = projectionState('(cat:1.20)', presentation);
  const first = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(first) }).state;
  state = state.update(exactReplacement(state, 1, 4, 'dog')).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ effects: installProjection.of(first) }).state;
  assert.notEqual(state.field(richProjection), first);
  const current = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(current) }).state;
  const depth = undoDepth(state);
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(false)),
  }).state;
  assert.deepEqual(projectionRanges(state), []);
  state = state.update({ effects: installProjection.of(current) }).state;
  assert.deepEqual(projectionRanges(state), []);
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(true)),
  }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  assert.equal(undoDepth(state), depth);
  assert.equal(state.doc.toString(), '(dog:1.20)');
  const other = projectionState('(dog:1.20)').update({
    effects: installProjection.of(current),
  }).state;
  assert.equal(other.field(richProjection).sourceIdentity, null);
});

test('malformed shells remain source text; exact newline-only changes retire prepared rendering', () => {
  let state = projectionState('x\n(cat:1.20)');
  const first = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(first) }).state;
  state = state.update(exactReplacement(state, 1, 2, '\r\n')).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ effects: installProjection.of(first) }).state;
  assert.notEqual(state.field(richProjection), first);
  const malformed = projectionState('(cat:bad), (unfinished');
  assert.equal(prepareProjection(malformed, prepareNativeStructure(malformed)).decorations.size, 0);
  for (const source of ['', 'ordinary', '(cat:bad), (unfinished']) {
    const plain = projectionState(source);
    const ready = plain.update({
      effects: installProjection.of(prepareProjection(plain, prepareNativeStructure(plain))),
    }).state;
    assert.equal(ready.field(richProjection).sourceIdentity, plain.field(editorFreshness).source);
  }
});

test('concealed-source selection exposes only its mapped token and reuses output until selection leaves', () => {
  let state = projectionState('(cat:1.20), (dog:2.00)');
  const prepared = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(prepared) }).state;
  state = state.update({ selection: { anchor: 8, head: 7 } }).state;
  const exposed = state.field(richProjection);
  assert.deepEqual(projectionRanges(state), [
    [12, 13],
    [16, 22],
  ]);
  assert.equal(exposed.all, prepared.all);
  assert.equal(exposed.tokens, prepared.tokens);
  assert.equal(undoDepth(state), 0);
  state = state.update({ selection: { anchor: 7, head: 6 } }).state;
  assert.equal(state.field(richProjection), exposed);
  state = state.update({ changes: { from: 0, insert: '🙂\n' } }).state;
  assert.deepEqual(projectionRanges(state), [
    [15, 16],
    [19, 25],
  ]);
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ selection: { anchor: state.doc.length } }).state;
  assert.equal(state.field(richProjection).decorations, state.field(richProjection).all);
  assert.deepEqual(projectionRanges(state), [
    [3, 4],
    [7, 13],
    [15, 16],
    [19, 25],
  ]);
});

test('escaped glyphs map exact CRLF source and expose only a selected concealed pair', () => {
  let state = projectionState('🙂\r\n\\(cat\\)');
  const prepared = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(prepared) }).state;
  assert.deepEqual(projectionRanges(state), [
    [3, 5],
    [8, 10],
  ]);
  state = state.update({ selection: { anchor: 4, head: 3 } }).state;
  assert.deepEqual(projectionRanges(state), [[8, 10]]);
  assert.equal(state.field(richProjection).all, prepared.all);
  assert.equal(undoDepth(state), 0);
  state = state.update({ selection: { anchor: 5 } }).state;
  assert.equal(state.field(richProjection).decorations, prepared.all);
});

test('feature permission invalidates emphasis while retaining independent rich escape semantics', () => {
  const profile = new Compartment();
  const source = String.raw`\(cat\), (dog:1.20)`;
  let state = projectionState(source);
  state = state.update({
    effects: StateEffect.appendConfig.of(
      profile.of(featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' }))),
    ),
  }).state;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  assert.equal(projectionRanges(state).length, 4);
  const enabled = state.field(richProjection);
  state = state.update({
    effects: profile.reconfigure(
      featureProfile.of(
        resolveFeatureProfile(
          {
            ...defaultPreferences,
            allowed: { ...defaultPreferences.allowed, emphasis: false },
          },
          { mode: 'library' },
        ),
      ),
    ),
  }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({ effects: installProjection.of(enabled) }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  assert.equal(projectionRanges(state).length, 0);
  const disabled = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(disabled) }).state;
  assert.deepEqual(projectionRanges(state), [
    [0, 2],
    [5, 7],
  ]);
  assert.equal(undoDepth(state), 0);
});

test('CSV rich source ranges map quoted CRLF, doubled quotes and nonBMP values without header or storage projection', () => {
  const source =
    '"(header:9)","{header}"\r\n"  🙂(a""b"":1.20), {animal|2}, <lora:model:0.5>, \\(x\\)\r\n[SEP|R]\r\n(cat:2)  ","(dog:3)"';
  let state = projectionState(source, new Compartment(), 'wildcard-csv');
  const canonical = prepareNativeStructure(state);
  const prepared = prepareProjection(state, canonical);
  const wrappers: number[][] = [],
    weights: string[] = [];
  const wildcards: WildcardWidget[] = [],
    loras: LoraWidget[] = [];
  prepared.all.between(0, state.doc.length, (from, to, decoration) => {
    const widget = decoration.spec.widget;
    if (widget instanceof SourceWidget) wrappers.push([from, to]);
    if (widget instanceof EmphasisWidget && widget.part === 'suffix') weights.push(widget.weight);
    if (widget instanceof WildcardWidget) wildcards.push(widget);
    if (widget instanceof LoraWidget) loras.push(widget);
    assert.ok(
      canonical.nativeValues.entries.some(
        ({ semanticMapping }) =>
          semanticMapping &&
          from >= semanticMapping.logicalRange.from &&
          to <= semanticMapping.logicalRange.to,
      ),
    );
  });
  assert.deepEqual(wrappers, [
    [29, 30],
    [36, 42],
    [44, 45],
    [51, 54],
    [56, 72],
    [74, 76],
    [77, 79],
    [80, 87],
    [88, 89],
    [92, 95],
    [100, 101],
    [104, 107],
  ]);
  assert.deepEqual(weights, ['1.20', '2', '3']);
  assert.equal(wildcards.length, 2);
  assert.equal(wildcards[0]!.group, canonical.scopes.scopes[0]!.wildcardGroups[0]);
  assert.equal(wildcards[1]!.group, canonical.scopes.scopes[0]!.wildcardGroups[0]);
  assert.equal(loras.length, 1);
  assert.equal(loras[0]!.weight, '0.5');
  assert.equal(prepared.hasLoras, true);
  const tokens: number[][] = [];
  prepared.tokens.between(0, state.doc.length, (from, to) => {
    tokens.push([from, to]);
  });
  assert.deepEqual(tokens, [
    [29, 42],
    [44, 54],
    [56, 72],
    [74, 76],
    [77, 79],
    [80, 87],
    [88, 95],
    [100, 107],
  ]);
  assert.equal(prepared.allAtomic.size, wrappers.length);
  state = state.update({
    effects: installProjection.of(prepared),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  state = state.update({ selection: { anchor: 46, head: 49 } }).state;
  assert.equal(state.field(richProjection), prepared);
  assert.equal(readExactSource(state), source);
  assert.equal(undoDepth(state), 0);
});

test('CSV regions retain independent local ordinals, actual canonical targets and clipped neighbor-free rails', () => {
  let state = projectionState(
    'left,right\n"[SEP|A]\n(cat:2)  ","[SEP|B]\n(dog:3)"',
    new Compartment(),
    'wildcard-csv',
  );
  const canonical = prepareNativeStructure(state),
    prepared = prepareProjection(state, canonical);
  const widgets: RegionWidget[] = [];
  const rails: number[][] = [];
  prepared.all.between(0, state.doc.length, (from, to, decoration) => {
    if (decoration.spec.widget instanceof RegionWidget) widgets.push(decoration.spec.widget);
    if (decoration.spec.attributes?.class === 'sp-region-content') rails.push([from, to]);
  });
  assert.deepEqual(
    widgets.map((widget) => [widget.target.scopeId, widget.ordinal]),
    [
      ['csv-container:1:0', 0],
      ['csv-container:1:1', 0],
    ],
  );
  assert.equal(widgets[0]!.target, canonical.regions[0]);
  assert.equal(widgets[1]!.target, canonical.regions[1]);
  assert.deepEqual(rails, [
    [20, 27],
    [40, 47],
  ]);
  assert.deepEqual(Object.keys(prepared.regionHovers), ['csv-container:1:0', 'csv-container:1:1']);
  for (const [index, scopeId] of Object.keys(prepared.regionHovers).entries()) {
    const hover = prepared.regionHovers[scopeId]![0]!;
    assert.equal(hover.target, canonical.regions[index]);
    const ranges: number[][] = [];
    hover.decorations.between(0, state.doc.length, (from, to) => {
      ranges.push([from, to]);
    });
    assert.deepEqual(
      ranges,
      index === 0
        ? [
            [12, 19],
            [20, 27],
          ]
        : [
            [32, 39],
            [40, 47],
          ],
    );
  }
  state = state.update({ effects: installProjection.of(prepared) }).state;
  state = state.update({ selection: { anchor: 22 } }).state;
  assert.equal(state.field(richProjection).regionHovers, prepared.regionHovers);
});
