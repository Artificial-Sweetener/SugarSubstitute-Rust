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
/** Passive mounted-view observer propagation, disposal and failure ownership remain separate from semantic selection proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { PromptMouseSelection } from '../../../../../../frontend/sugarprompt/editor/src/mouse-selection.ts';
import { heldProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';
test('replacement capture precedes native content admission and bubbling cannot release the newly admitted hold', () => {
  const fixture = heldTokenFixture();
  try {
    const first = fixture.press();
    fixture.select(first.style, first.event);
    const old = fixture.state.field(heldProjection)!.session;
    const secondEvent = new Event('mousedown');
    fixture.document.phase('mousedown', secondEvent, true);
    assert.equal(fixture.state.field(heldProjection), null);
    const second = fixture.press();
    fixture.select(second.style, second.event);
    const current = fixture.state.field(heldProjection)!.session;
    fixture.document.phase('mousedown', secondEvent, false);
    assert.notEqual(current, old);
    assert.equal(fixture.state.field(heldProjection)!.session, current);
    fixture.select(second.style, heldTokenPointer(22));
    fixture.document.phase('mouseup', new Event('mouseup'), false);
    // Native double-click release does not reselect; a primary-selection click may query its last event after release.
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [6, 25],
    );
    const click = fixture.press(heldTokenPointer(8, 1));
    fixture.document.phase('mouseup', new Event('mouseup'), false);
    fixture.select(click.style, click.event);
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [8, 8],
    );
  } finally {
    fixture.destroy();
  }
});

test('retired and superseded style callbacks consume captured native selection without coordinate or retired-view reads', () => {
  const fixture = heldTokenFixture();
  const first = fixture.press();
  fixture.select(first.style, first.event);
  const expected = fixture.state.selection;
  fixture.press();
  fixture.duringRead(() => assert.fail('superseded style cannot read coordinates'));
  assert.ok(first.style.get(heldTokenPointer(22), false, false).eq(expected));
  fixture.destroy();
  assert.ok(first.style.get(heldTokenPointer(22), false, false).eq(expected));
  first.style.update({} as never);
});

test('partial observer admission attempts every removal and preserves undefined primary plus independent cleanup causes', () => {
  let additions = 0;
  const removals: string[] = [];
  const cleanup = Symbol('cleanup');
  const document = {
    addEventListener() {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      if (++additions === 3) throw undefined;
    },
    removeEventListener(type: string) {
      removals.push(type);
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      if (type === 'mouseup') throw cleanup;
    },
    defaultView: {
      removeEventListener(type: string) {
        removals.push(type);
      },
    },
  };
  let failure: unknown,
    failed = false;
  try {
    new PromptMouseSelection({
      contentDOM: { ownerDocument: document },
    } as unknown as EditorView);
  } catch (cause) {
    failed = true;
    failure = cause;
  }
  assert.equal(failed, true);
  assert.ok(failure instanceof AggregateError);
  assert.deepEqual(failure.errors, [undefined, cleanup]);
  assert.deepEqual(removals, ['mouseup', 'mousemove', 'mousedown', 'pointercancel', 'blur']);
});
