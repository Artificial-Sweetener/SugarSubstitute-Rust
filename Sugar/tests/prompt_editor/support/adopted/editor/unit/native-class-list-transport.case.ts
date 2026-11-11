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
/** Both existing native paint transports expose literal class tokens without simulating feedback semantics. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { PaintNode } from './reorder-paint-node-fixture.ts';
import { fixture } from './reorder-session-fixture.ts';

for (const kind of ['paint', 'session'] as const)
  test(`${kind} native token transport follows className and class attributes`, () => {
    const session = kind === 'session' ? fixture() : null,
      failures: unknown[] = [];
    try {
      const node = session
          ? session.view.dom.ownerDocument.createElement('span')
          : new PaintNode('span', {}, { left: 0, top: 0, width: 0, height: 0 }),
        tokens = node.classList;
      assert.equal(node.classList, tokens);
      assert.equal(tokens.contains(''), false);
      assert.equal(tokens.contains('two tokens'), false);
      node.className = 'base\tretained';
      assert.equal(tokens.contains('retained'), true);
      assert.equal(tokens.toggle('accent', true), true);
      assert.equal(node.className, 'base retained accent');
      assert.equal(tokens.toggle('accent', true), true);
      assert.equal(node.className, 'base retained accent');
      assert.equal(tokens.toggle('accent', false), false);
      assert.equal(node.className, 'base retained');
      assert.equal(tokens.toggle('accent'), true);
      assert.equal(tokens.toggle('accent'), false);
      node.className = 'replacement';
      assert.equal(tokens.contains('base'), false);
      assert.equal(tokens.contains('replacement'), true);
      assert.equal(node.getAttribute('class'), 'replacement');
      assert.equal(tokens.toggle('replacement', false), false);
      assert.equal(node.className, '');
      assert.throws(() => tokens.toggle(''), { name: 'SyntaxError' });
      assert.throws(() => tokens.toggle('two tokens'), {
        name: 'InvalidCharacterError',
      });
      assert.equal(node.className, '');
    } catch (cause) {
      failures.push(cause);
    }
    try {
      session?.destroy();
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'native-class-list-transport-failed');
  });
