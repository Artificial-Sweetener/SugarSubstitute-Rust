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
/** Finite owner admission fences browser drag defaults without substituting for trusted native proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { fixture, key, pointer } from './reorder-session-fixture.ts';

test('primary Alt pointer supersedes pending keyboard entry and fences native drag defaults through capture', () => {
  const f = fixture();
  const selection = f.state.selection;
  try {
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.owner.keydown(key('Alt')), true);
    assert.equal(f.owner.blocksInput(), true);
    assert.equal(f.owner.pointerdown(pointer(7)), true);
    assert.equal(f.owner.blocksInput(), true);
    f.measure();
    f.task();
    assert.equal(f.captures.has(7), true);
    assert.equal(f.state.field(reorderPreview).value !== null, true);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection.eq(selection), true);
    assert.equal(undoDepth(f.state), 0);
    f.owner.close();
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('active primary Alt gap presses consume native caret/drag defaults while dormant ordinary presses delegate', () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'Element');
  class ElementTransport {}
  Object.defineProperty(globalThis, 'Element', {
    configurable: true,
    value: ElementTransport,
  });
  const f = fixture();
  try {
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    assert.equal(f.owner.pointerdown({ ...pointer(), altKey: false }), false);
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const before = f.state;
    assert.equal(f.owner.pointerdown(pointer()), true);
    assert.equal(f.state, before);
    assert.equal(f.captures.size, 0);
    assert.equal(f.owner.blocksInput(), true);
    f.owner.close();
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    assert.equal(f.owner.pointerdown({ ...pointer(), altKey: false }), false);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
    if (descriptor) Object.defineProperty(globalThis, 'Element', descriptor);
    else Reflect.deleteProperty(globalThis, 'Element');
  }
});
