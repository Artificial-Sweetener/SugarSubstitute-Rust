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
/** Native prepared ranges prove canonical glyph ownership, source/history preservation and bounded reveal/reuse. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, StateEffect, type EditorState } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { prepareWildcardCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { wildcardCatalog } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-catalog.ts';
import { WildcardColumnWidget } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-column-widget.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { projectionState } from './projection-state.ts';

test('CSV column glyph aliases map admitted decoded fields after doubled quotes, CRLF and nonBMP text', () => {
  const source = 'left,right\r\n"🙂a""b""\r\n{csv:aliases:name}","{csv:aliases:name}"';
  let state = projectionState(source, new Compartment(), 'wildcard-csv').update({
    effects: StateEffect.appendConfig.of(
      wildcardCatalog.of(
        prepareWildcardCatalog({
          status: 'ready',
          revision: 'csv-alias',
          textIdentifiers: [],
          csvSources: [{ identifier: 'aliases', columns: ['NaMe'] }],
        }),
      ),
    ),
  }).state;
  const projection = prepareProjection(state, prepareNativeStructure(state));
  state = state.update({ effects: installProjection.of(projection) }).state;
  const first = state.doc.toString().indexOf('name'),
    second = state.doc.toString().lastIndexOf('name');
  assert.deepEqual(aliases(state), [
    { from: first, to: first + 1, text: 'N' },
    { from: first + 2, to: first + 3, text: 'M' },
    { from: second, to: second + 1, text: 'N' },
    { from: second + 2, to: second + 3, text: 'M' },
  ]);
  assert.equal(readExactSource(state), source);
  assert.equal(undoDepth(state), 0);
});

/** Publish actual source-current projection with a reactive canonical-column catalog. */
function prepared(source: string): {
  readonly state: EditorState;
  readonly catalog: Compartment;
} {
  const catalog = new Compartment();
  let state = projectionState(source).update({
    effects: StateEffect.appendConfig.of(
      catalog.of(
        wildcardCatalog.of(
          prepareWildcardCatalog({
            status: 'ready',
            revision: 'fixture',
            textIdentifiers: [],
            csvSources: [{ identifier: 'aliases', columns: ['NaMe', 'İdea'] }],
          }),
        ),
      ),
    ),
  }).state;
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  return { state, catalog };
}

/** Observe current native alias ranges in logical-document UTF-16, without constructing browser DOM. */
function aliases(state: EditorState): readonly {
  readonly from: number;
  readonly to: number;
  readonly text: string;
}[] {
  const result: { from: number; to: number; text: string }[] = [];
  state.field(richProjection).decorations.between(0, state.doc.length, (from, to, value) => {
    if (value.spec.widget instanceof WildcardColumnWidget)
      result.push({ from, to, text: value.spec.widget.text });
  });
  return result;
}

test('prepared canonical glyphs use logical offsets while the actual native projection retains exact source and history', () => {
  const source = '🙂\r\n{csv:aliases:name|2}, {csv:aliases:i\u0307dea}',
    fixture = prepared(source),
    name = source.indexOf('name') - 1,
    dot = source.indexOf('i\u0307dea') - 1;
  assert.deepEqual(aliases(fixture.state), [
    { from: name, to: name + 1, text: 'N' },
    { from: name + 2, to: name + 3, text: 'M' },
    { from: dot, to: dot + 2, text: 'İ' },
  ]);
  assert.equal(readExactSource(fixture.state), source);
  assert.equal(undoDepth(fixture.state), 0);
  const projection = fixture.state.field(richProjection),
    atomic: { from: number; to: number }[] = [];
  projection.atomic.between(0, fixture.state.doc.length, (from, to, value) => {
    assert.equal(value.point, true);
    atomic.push({ from, to });
  });
  assert.deepEqual(atomic, [
    { from: 3, to: 8 },
    { from: name, to: name + 1 },
    { from: name + 2, to: name + 3 },
    { from: name + 4, to: name + 7 },
    { from: name + 9, to: dot - 8 },
    { from: dot, to: dot + 2 },
    { from: dot + 5, to: dot + 6 },
  ]);
  const all = fixture.state.field(richProjection).all;
  const state = fixture.state.update({
    selection: { anchor: source.length - 1, head: name },
  }).state;
  assert.equal(state.field(richProjection).all, all);
  assert.equal(state.field(richProjection).atomic, projection.atomic);
  assert.equal(state.selection.main.anchor, source.length - 1);
  assert.equal(state.selection.main.head, name);
});

test('an interior expanded-scalar caret reveals only its source token and leaving it restores the actual prepared projection', () => {
  const source = '{csv:aliases:i\u0307dea}, {csv:aliases:name}',
    fixture = prepared(source),
    all = fixture.state.field(richProjection).all;
  const inside = fixture.state.update({
    selection: { anchor: source.indexOf('i\u0307dea') + 1 },
  }).state;
  assert.equal(inside.field(richProjection).expanded.length, 1);
  assert.equal(inside.field(richProjection).atomic.size, 4);
  assert.deepEqual(
    aliases(inside).map((alias) => alias.text),
    ['N', 'M'],
  );
  const outside = inside.update({ selection: { anchor: 0 } }).state;
  assert.equal(outside.field(richProjection).decorations, all);
  assert.equal(outside.field(richProjection).atomic, fixture.state.field(richProjection).allAtomic);
  assert.deepEqual(
    aliases(outside).map((alias) => alias.text),
    ['İ', 'N', 'M'],
  );
  assert.equal(readExactSource(outside), source);
  assert.equal(undoDepth(outside), 0);
});

test('catalog context rejects old alias publication and incomplete replacement prepares literal source without history changes', () => {
  const source = '{csv:aliases:name}',
    fixture = prepared(source),
    old = prepareProjection(fixture.state, prepareNativeStructure(fixture.state));
  let state = fixture.state.update({
    effects: [
      fixture.catalog.reconfigure(
        wildcardCatalog.of(prepareWildcardCatalog({ status: 'pending', revision: 'pending' })),
      ),
      invalidateCommandContext.of(null),
    ],
  }).state;
  state = state.update({ effects: installProjection.of(old) }).state;
  assert.equal(state.field(richProjection).sourceIdentity, null);
  state = state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
  assert.deepEqual(aliases(state), []);
  assert.equal(readExactSource(state), source);
  assert.equal(undoDepth(state), 0);
});
