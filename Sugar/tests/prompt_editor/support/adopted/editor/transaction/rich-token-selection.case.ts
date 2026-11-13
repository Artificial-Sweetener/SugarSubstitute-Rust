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
/** Complete scene/wildcard selection retains prepared body geometry; emphasis alone selects its visible body. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';
import { heldProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

for (const [name, source, hit, bodyFrom, bodyTo, outerFrom, outerTo] of [
  ['scene hidden markers', ' \t**  Wide scene \t\r\nbody', 8, 6, 16, 0, 18],
  ['CSV column and hidden group', '🙂\r\n{csv:aliases:name|2}\rtail', 12, 8, 20, 3, 23],
  ['canonical grouped wildcard', '{csv:folder/animal:Color|2}\r\ntail', 8, 5, 24, 0, 27],
  ['Unicode scene after CRLF', '🙂\r\n \t**  Wide 🙂 scene \t\r\nbody', 12, 9, 22, 3, 24],
] as const)
  test(`${name} double-click selects the complete source token while preserving semantic body geometry`, () => {
    const fixture = heldTokenFixture(source);
    try {
      const bodies: number[][] = [];
      fixture.state
        .field(semanticStructures)
        .native!.pointerTargets.between(hit, hit, (from, _to, value) => {
          if (value.kind === 'scene' || value.kind === 'wildcard')
            bodies.push([from + value.contentFrom, from + value.contentTo]);
        });
      assert.deepEqual(bodies, [[bodyFrom, bodyTo]]);
      const { event, style } = fixture.press(heldTokenPointer(hit));
      fixture.select(style, event);
      assert.deepEqual(
        [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
        [outerFrom, outerTo],
      );
      assert.deepEqual(fixture.state.field(heldProjection)?.range, {
        from: outerFrom,
        to: outerTo,
      });
      fixture.document.dispatchEvent(new Event('mouseup'));
      assert.equal(fixture.state.field(heldProjection), null);
      assert.deepEqual(
        [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
        [outerFrom, outerTo],
      );
      assert.equal(readExactSource(fixture.state), source);
      assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
    } finally {
      fixture.destroy();
    }
    assert.deepEqual([fixture.document.active, fixture.window.active], [0, 0]);
  });

test('nested emphasis retains its smallest visible body selection and exact outer exposure', () => {
  const fixture = heldTokenFixture('(outer (inner:1.2):1.3)');
  try {
    const { event, style } = fixture.press(heldTokenPointer(10));
    fixture.select(style, event);
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [8, 13],
    );
    assert.deepEqual(fixture.state.field(heldProjection)?.range, {
      from: 7,
      to: 18,
    });
    assert.equal(undoDepth(fixture.state), 0);
  } finally {
    fixture.destroy();
  }
  assert.deepEqual([fixture.document.active, fixture.window.active], [0, 0]);
});

test('wildcard source drag preserves reverse and forward anchors while outer exposure survives until release', () => {
  const source = 'lead {csv:folder/animal:Color|2}, tail';
  const fixture = heldTokenFixture(source);
  try {
    const { event, style } = fixture.press(heldTokenPointer(12));
    fixture.select(style, event);
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [5, 32],
    );
    fixture.select(style, heldTokenPointer(1));
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [32, 0],
    );
    fixture.select(style, heldTokenPointer(35));
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [5, 38],
    );
    assert.deepEqual(fixture.state.field(heldProjection)?.range, {
      from: 5,
      to: 32,
    });
    fixture.document.dispatchEvent(new Event('mouseup'));
    assert.equal(fixture.state.field(heldProjection), null);
    assert.equal(readExactSource(fixture.state), source);
    assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
  } finally {
    fixture.destroy();
  }
});
