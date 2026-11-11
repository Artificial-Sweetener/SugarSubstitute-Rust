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
/** Actual CM fields prove canonical diagnostic preparation, selectable projection and source/context retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, EditorState, StateEffect } from '@codemirror/state';
import { refreshFixture } from '../unit/semantic-refresh-fixture.ts';
import { structureState } from './document-structure-fixture.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  duplicateDiagnostics,
  prepareNativeDuplicates,
  installNativeDuplicates,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-diagnostics.ts';
import {
  duplicateIgnores,
  ignoreDuplicate,
} from '../../../../../../frontend/sugarprompt/editor/src/duplicate-ignore.ts';
import { duplicateMenuTargetAt } from '../../../../../../frontend/sugarprompt/editor/src/duplicate-menu.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { conditioningContext } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-context.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Publish the sole genuine structure and actual duplicate owner outputs in one source-neutral transaction. */
function ready(state: ReturnType<typeof structureState>['state']) {
  const prepared = prepareNativeStructure(state);
  return state.update({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared }),
      installNativeDuplicates.of(prepareNativeDuplicates(state, prepared.nativeValues)),
    ],
  }).state;
}

test('CRLF and CSV quoted values project actual source marks without diagnosing headers or adjacent values', () => {
  const f = structureState('label,value\r\nrow,"cat, cat"\r\nrow2,cat', 'wildcard-csv', [
    duplicateIgnores,
    duplicateDiagnostics,
  ]);
  const state = ready(f.state),
    value = state.field(duplicateDiagnostics);
  assert.equal(readExactSource(state), 'label,value\r\nrow,"cat, cat"\r\nrow2,cat');
  assert.equal(value.prepared!.findings.length, 1);
  const target = duplicateMenuTargetAt(state, 22)!;
  assert.deepEqual([target.from, target.to], [22, 25]);
  assert.deepEqual(target.finding.diagnostic.range, { from: 23, to: 26 });
  let mark = false;
  value.decorations.between(22, 25, (from, to, decoration) => {
    assert.deepEqual([from, to], [22, 25]);
    assert.equal(decoration.spec.class, 'sp-duplicate-segment');
    assert.equal(decoration.spec.widget, undefined);
    mark = true;
  });
  assert.equal(mark, true);
  assert.equal(prepareNativeDuplicates(state, prepareNativeStructure(state).nativeValues), value);
  assert.equal(
    duplicateMenuTargetAt(state.update({ selection: EditorSelection.single(25) }).state, 22)
      ?.finding,
    target.finding,
  );
});

test('actual duplicate output and menus remain visible under interior and end caret without re-preparing findings', () => {
  let state = ready(
    structureState('cat, cat, tail', 'prompt', [duplicateIgnores, duplicateDiagnostics]).state,
  );
  const projection = state.field(duplicateDiagnostics),
    prepared = projection.prepared;
  assert.ok(duplicateMenuTargetAt(state, 5));
  state = state.update({ selection: EditorSelection.single(6) }).state;
  assert.equal(state.field(duplicateDiagnostics).prepared, prepared);
  assert.ok(duplicateMenuTargetAt(state, 5));
  assert.equal(state.field(duplicateDiagnostics), projection);
  state = state.update({ selection: EditorSelection.single(8) }).state;
  assert.ok(duplicateMenuTargetAt(state, 5));
  state = ready(
    structureState('cat, cat', 'prompt', [duplicateIgnores, duplicateDiagnostics]).state,
  ).update({ selection: EditorSelection.single(8) }).state;
  assert.ok(duplicateMenuTargetAt(state, 5));
});

test('ignore range/key identity survives edits and profile refresh while current findings and menus retire', () => {
  const slot = new Compartment();
  const on = resolveFeatureProfile(defaultPreferences, { mode: 'library' });
  const off = resolveFeatureProfile(
    {
      ...defaultPreferences,
      allowed: {
        ...defaultPreferences.allowed,
        duplicate_segment_diagnostics: false,
      },
    },
    { mode: 'library' },
  );
  let state = ready(
    structureState('cat, cat, tail', 'prompt', [
      duplicateIgnores,
      duplicateDiagnostics,
      slot.of(featureProfile.of(on)),
    ]).state,
  );
  const finding = duplicateMenuTargetAt(state, 5)!.finding;
  state = state.update({
    effects: ignoreDuplicate.of(finding.ignoreKey),
  }).state;
  assert.equal(duplicateMenuTargetAt(state, 5), null);
  assert.equal(state.field(duplicateDiagnostics).decorations.size, 0);
  const ignored = state.field(duplicateIgnores);
  state = state.update(exactReplacement(state, 10, 14, 'bird')).state;
  assert.equal(state.field(duplicateDiagnostics).prepared, null);
  assert.throws(
    () =>
      prepareNativeDuplicates(
        state,
        prepareNativeStructure(
          ready(
            structureState('cat, cat, tail', 'prompt', [duplicateIgnores, duplicateDiagnostics])
              .state,
          ),
        ).nativeValues,
      ),
    { message: 'duplicate-structure-currentness' },
  );
  state = ready(state);
  assert.equal(state.field(duplicateIgnores), ignored);
  assert.equal(duplicateMenuTargetAt(state, 5), null);
  state = state.update({
    effects: slot.reconfigure(featureProfile.of(off)),
  }).state;
  assert.equal(state.field(duplicateDiagnostics).prepared, null);
  state = ready(state);
  assert.equal(state.field(duplicateDiagnostics).prepared, null);
  assert.equal(state.field(duplicateIgnores), ignored);
  state = ready(state.update({ effects: slot.reconfigure(featureProfile.of(on)) }).state);
  assert.equal(state.field(duplicateDiagnostics).prepared!.findings.length, 1);
  assert.equal(duplicateMenuTargetAt(state, 5), null);
});

test('mode currentness, read-only refresh and genuine preparation reject old or copied publications', () => {
  const mode = new Compartment();
  const f = structureState('cat\n[SEP]\ncat', 'prompt', [
    duplicateIgnores,
    duplicateDiagnostics,
    mode.of(conditioningContext.of({ key: 'regional', mode: 'regional' })),
  ]);
  let state = ready(f.state);
  const old = state.field(duplicateDiagnostics);
  assert.equal(old.prepared!.findings.length, 1);
  state = state.update({
    effects: mode.reconfigure(conditioningContext.of({ key: 'independent', mode: 'independent' })),
  }).state;
  state = state.update({ effects: installNativeDuplicates.of(old) }).state;
  assert.equal(state.field(duplicateDiagnostics).freshness, null);
  state = ready(state);
  assert.equal(state.field(duplicateDiagnostics).prepared!.findings.length, 0);
  const actual = prepareNativeDuplicates(state, prepareNativeStructure(state).nativeValues);
  state = state.update({
    effects: installNativeDuplicates.of({ ...actual, findings: old.findings }),
  }).state;
  assert.equal(state.field(duplicateDiagnostics).prepared!.findings.length, 0);
  state = state.update({
    effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(state.field(duplicateDiagnostics).freshness, null);
  assert.ok(ready(state).field(duplicateDiagnostics).freshness);
});

test('the actual measured refresh publishes duplicates from its same canonical native preparation and retires pending teardown', () => {
  const f = refreshFixture();
  try {
    f.jobs[0]!.read();
    f.jobs[0]!.write();
    f.task();
    f.apply(
      f.state().update({
        ...exactReplacement(f.state(), 0, f.state().doc.length, 'cat, cat, tail'),
        effects: StateEffect.appendConfig.of([duplicateIgnores, duplicateDiagnostics]),
      }),
    );
    assert.equal(f.state().field(duplicateDiagnostics).freshness, null);
    f.jobs[1]!.read();
    f.jobs[1]!.write();
    assert.equal(f.state().field(duplicateDiagnostics).prepared, null);
    f.task();
    assert.ok(duplicateMenuTargetAt(f.state(), 5));
    assert.equal(
      f.state().field(duplicateDiagnostics).prepared!.scopes,
      prepareNativeStructure(f.state()).scopes,
    );
    const publications = f.publications();
    f.apply(f.state().update(exactReplacement(f.state(), 0, 3, 'dog')));
    f.jobs[2]!.read();
    f.jobs[2]!.write();
    f.owner.destroy();
    f.task();
    assert.equal(f.publications(), publications);
    assert.equal(f.state().field(duplicateDiagnostics).freshness, null);
  } finally {
    f.destroy();
  }
});

test('genuine late and same-transaction installs reconcile marks with actual current ignores', () => {
  for (const installFirst of [false, true]) {
    let state = structureState('cat, cat, tail', 'prompt', [
      duplicateIgnores,
      duplicateDiagnostics,
    ]).state;
    const native = prepareNativeStructure(state);
    const issued = prepareNativeDuplicates(state, native.nativeValues);
    state = state.update({
      effects: [
        installSemanticStructure.of({ purpose: 'native', prepared: native }),
        installNativeDuplicates.of(issued),
      ],
    }).state;
    const target = duplicateMenuTargetAt(state, 5)!;
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 1);
    const effects = [
      ignoreDuplicate.of(target.finding.ignoreKey),
      installNativeDuplicates.of(issued),
    ];
    state = state.update({
      effects: installFirst ? [...effects].reverse() : effects,
    }).state;
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 0);
    assert.equal(state.field(duplicateDiagnostics).baseDecorations.size, 1);
    const ignored = state.field(duplicateIgnores),
      before = state;
    state = state.update({ effects: installNativeDuplicates.of(issued) }).state;
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 0);
    assert.equal(state.field(duplicateIgnores), ignored);
    assert.equal(state.doc, before.doc);
    assert.equal(state.selection, before.selection);
    assert.equal(readExactSource(state), 'cat, cat, tail');
    assert.equal(duplicateMenuTargetAt(state, 5), null);
    const reused = prepareNativeDuplicates(state, native.nativeValues);
    assert.equal(reused, state.field(duplicateDiagnostics));
    state = state.update({ effects: installNativeDuplicates.of(reused) }).state;
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 0);
    state = state.update({ selection: EditorSelection.single(6) }).state;
    assert.equal(state.field(duplicateDiagnostics).decorations.size, 0);
    assert.equal(state.field(duplicateDiagnostics).baseDecorations.size, 1);
  }
});
