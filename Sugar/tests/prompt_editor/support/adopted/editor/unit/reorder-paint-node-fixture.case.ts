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
/** Literal native parentage must remain a tree when production copy writers move existing nodes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { PaintNode, PaintText } from './reorder-paint-node-fixture.ts';

/** Independent literal geometry and one external document suffice for parentage transport. */
function parentageFixture() {
  const document = {};
  const create = (name: string) =>
    new PaintNode(name, document, { left: 0, top: 0, width: 10, height: 10 });
  const body = create('body'),
    first = create('span'),
    second = create('span');
  body.append(first, second);
  return { create, first, second };
}

test('append transfers existing element and text identities without leaving stale old-parent membership', () => {
  const { create, first, second } = parentageFixture();
  const element = create('span'),
    text = new PaintText('literal');
  first.append(element, text);
  second.append(text, element);
  assert.deepEqual(first.childNodes, []);
  assert.deepEqual(second.childNodes, [text, element]);
  assert.equal(text.parent, second);
  assert.equal(element.parentElement, second);
  assert.equal(text.isConnected, true);
  assert.equal(element.isConnected, true);
});

test('append moves an existing sibling to the end once and preserves multiple-argument native order', () => {
  const { create, first } = parentageFixture();
  const a = create('span'),
    b = create('span'),
    c = create('span');
  first.append(a, b, c);
  first.append(a);
  assert.deepEqual(first.childNodes, [b, c, a]);
  first.append(c, b);
  assert.deepEqual(first.childNodes, [a, c, b]);
  assert.equal(new Set(first.childNodes).size, 3);
});

test('moving into a borrowed span then replacing the original parent preserves descendant connection', () => {
  const { create, first } = parentageFixture();
  const island = create('span'),
    borrowed = create('span');
  first.append(island);
  borrowed.append(island);
  first.replaceChildren(borrowed);
  assert.deepEqual(first.childNodes, [borrowed]);
  assert.deepEqual(borrowed.childNodes, [island]);
  assert.equal(island.parentElement, borrowed);
  assert.equal(island.isConnected, true);
});
