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
/** Real pointer style, native navigation/deletion, measured projection and CodeMirror history prove structural exposure rules. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { undo, redo } from '@codemirror/commands';
import { deleteGrapheme } from '../../../../../../frontend/sugarprompt/editor/src/input-policy.ts';
import { horizontalNavigation } from '../../../../../../frontend/sugarprompt/editor/src/native-navigation.ts';
import { richProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  prepareNativeSnapshot,
  materializeNativeSnapshot,
} from '../../../../../../frontend/sugarprompt/editor/src/native-snapshot.ts';
import { structuralExposureFixture } from './structural-exposure-fixture.ts';

/** Count genuine prepared source-backed replacements, never synthetic DOM or a copied visibility rule. */
function replacements(fixture: ReturnType<typeof structuralExposureFixture>): number {
  let count = 0;
  fixture
    .state()
    .field(richProjection)
    .decorations.between(0, fixture.state().doc.length, (_from, _to, value) => {
      if (value.spec.widget) count++;
    });
  return count;
}

test('actual scene double-click and public-style selection preserve concealed prefix deletion', () => {
  const source = ' \t**  Wide scene \t\r\nbody',
    fixture = structuralExposureFixture(source);
  try {
    fixture.press(9, 2);
    assert.deepEqual(
      [fixture.state().selection.main.anchor, fixture.state().selection.main.head],
      [0, 18],
    );
    fixture.release();
    assert.equal(replacements(fixture), 1);
    assert.equal(horizontalNavigation(fixture.view, true, false), true);
    fixture.press(14, 1);
    fixture.dispatch({
      selection: { anchor: 14, head: 8 },
      userEvent: 'select.pointer',
    });
    fixture.release();
    fixture.commands.selectSource(0, 18);
    fixture.commands.selectSource(6);
    assert.equal(replacements(fixture), 1);
    assert.equal(deleteGrapheme(fixture.view, false), true);
    assert.equal(readExactSource(fixture.state()) === source.slice(6), true);
    assert.equal(undo(fixture.view), true);
    fixture.settle();
    assert.equal(readExactSource(fixture.state()) === source, true);
  } finally {
    fixture.destroy();
  }
});

test('actual structural reveal survives lower-bound navigation, typing and associated-selection Undo', () => {
  const source = String.raw`((e\(lan\):1.20):1.30), tail`,
    fixture = structuralExposureFixture(source);
  try {
    fixture.commands.selectSource(source.indexOf(','));
    assert.equal(deleteGrapheme(fixture.view, false), true);
    assert.equal(replacements(fixture), 0);
    assert.equal(horizontalNavigation(fixture.view, false, false), true);
    assert.equal(fixture.state().selection.main.head, 0);
    assert.equal(replacements(fixture), 0);
    assert.equal(horizontalNavigation(fixture.view, true, false), true);
    assert.equal(fixture.state().selection.main.head, 1);
    fixture.dispatch({
      changes: { from: 1, insert: 'X' },
      selection: { anchor: 2 },
      userEvent: 'input.type',
    });
    fixture.settle();
    assert.equal(replacements(fixture), 0);
    fixture.commands.selectSource(source.length + 1);
    assert.ok(replacements(fixture) > 0);
    assert.equal(undo(fixture.view), true);
    fixture.settle();
    assert.equal(readExactSource(fixture.state()) === source, true);
    assert.equal(fixture.state().selection.main.head, 1);
    assert.equal(replacements(fixture), 0);
    assert.equal(redo(fixture.view), true);
    fixture.settle();
    assert.equal(readExactSource(fixture.state()) === '(X' + source.slice(1), true);
    assert.equal(fixture.state().selection.main.head, source.length + 1);
    assert.ok(replacements(fixture) > 0);
  } finally {
    fixture.destroy();
  }
});

test('public native snapshots retain identical state and projection while observing readiness', () => {
  const fixture = structuralExposureFixture('(cat:1.20)');
  try {
    const state = fixture.state(),
      projection = state.field(richProjection);
    for (let i = 0; i < 100; i++) materializeNativeSnapshot(prepareNativeSnapshot(fixture.view, 0));
    assert.equal(fixture.state(), state);
    assert.equal(fixture.state().field(richProjection), projection);
  } finally {
    fixture.destroy();
  }
});

for (const [position, retained] of [
  [5, true],
  [15, true],
  [4, false],
  [16, false],
] as const)
  test(`explicit raw exposure ${retained ? 'retains' : 'retires'} at native boundary coordinate ${position}`, () => {
    const fixture = structuralExposureFixture('lead\n(cat:1.20), tail');
    try {
      fixture.commands.selectSource(15);
      assert.equal(deleteGrapheme(fixture.view, false), true);
      fixture.commands.selectSource(position);
      assert.equal(fixture.state().field(richProjection).revealed !== null, retained);
      assert.equal(fixture.state().selection.main.head, position);
    } finally {
      fixture.destroy();
    }
  });
