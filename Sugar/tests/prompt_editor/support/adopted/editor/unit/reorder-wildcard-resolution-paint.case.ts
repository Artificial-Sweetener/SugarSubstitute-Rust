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
/** Readonly paint transport retains the complete closed resolution vocabulary while rejecting unrelated source attributes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { captureNativePaint } from './reorder-native-paint-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

test('actual prepared paint preserves wildcard pending and existing LoRA resolutions without admitting unknown resolution values', () => {
  for (const status of [
    'found',
    'missing',
    'ambiguous',
    'pending',
    'pending-authority',
    'unavailable',
    'unknown',
  ] as const) {
    const f = motionPaintFixture();
    f.chips[0]!.setAttribute('data-resolution', status);
    f.chips[0]!.setAttribute('id', 'source-owner');
    const copy = captureNativePaint(f);
    assert.ok(copy);
    try {
      const node = copy.querySelectorAll<HTMLElement>('.sp-reorder-chip')[0];
      assert.ok(node);
      assert.equal(node.getAttribute('data-resolution'), status === 'unknown' ? null : status);
      assert.equal(node.getAttribute('id'), null);
      assert.equal(f.chips[0]!.getAttribute('data-resolution'), status);
      assert.equal(f.line.textContent, 'abc');
    } finally {
      copy.remove();
    }
  }
});
