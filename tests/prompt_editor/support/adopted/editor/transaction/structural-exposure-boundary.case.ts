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
/** Real prepared rendering distinguishes scene pointer geometry from structural source authoring. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { heldProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import {
  richProjection,
  currentInputProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { deleteGrapheme } from '../../../../../../frontend/sugarprompt/editor/src/input-policy.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';
import { structuralExposureFixture } from './structural-exposure-fixture.ts';

test('scene title geometry retains its outer hold without exposing concealed markers', () => {
  const source = '🙂\r\n \t**  Wide 🙂 scene \t\r\nbody';
  const fixture = heldTokenFixture(source);
  try {
    const original = fixture.state.field(richProjection);
    const { event, style } = fixture.press(heldTokenPointer(12));
    fixture.select(style, event);
    assert.deepEqual(fixture.state.field(heldProjection)?.range, {
      from: 3,
      to: 24,
    });
    assert.equal(fixture.state.field(heldProjection)?.exposesSyntax, false);
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [3, 24],
    );
    const held = fixture.state.field(richProjection);
    assert.equal(held.revealed, null);
    assert.equal(held.retainRevealOpening, false);
    assert.equal(held.decorations, original.all);
    assert.equal(held.atomic, original.allAtomic);
    assert.equal(held.tokens, original.tokens);
    assert.equal(currentInputProjection(fixture.state)?.decorations, original.all);
    fixture.document.dispatchEvent(new Event('mouseup'));
    assert.equal(fixture.state.field(heldProjection), null);
    assert.equal(fixture.state.field(richProjection).decorations, original.all);
    assert.equal(readExactSource(fixture.state), source);
  } finally {
    fixture.destroy();
  }
  assert.deepEqual([fixture.document.active, fixture.window.active], [0, 0]);
});

test('native structural raw editing retains a Unicode CRLF token opening then retires outside it', () => {
  const source = '🙂\r\n(cat:1.20), tail';
  const fixture = structuralExposureFixture(source);
  try {
    fixture.commands.selectSource(14);
    assert.equal(deleteGrapheme(fixture.view, false), true);
    assert.deepEqual(fixture.state().field(richProjection).revealed, {
      from: 3,
      to: 13,
    });
    assert.equal(fixture.state().field(richProjection).retainRevealOpening, true);
    fixture.commands.selectSource(4);
    assert.equal(fixture.state().selection.main.head, 3);
    assert.deepEqual(fixture.state().field(richProjection).revealed, {
      from: 3,
      to: 13,
    });
    assert.equal(fixture.state().field(richProjection).retainRevealOpening, true);
    const raw = currentInputProjection(fixture.state());
    assert.ok(raw);
    assert.equal(raw.decorations.size, 0);
    fixture.commands.selectSource(2);
    assert.equal(fixture.state().field(richProjection).revealed, null);
    assert.equal(fixture.state().field(richProjection).retainRevealOpening, false);
    assert.equal(readExactSource(fixture.state()), source);
  } finally {
    fixture.destroy();
  }
});
