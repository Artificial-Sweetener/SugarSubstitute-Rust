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
/** Independent CSSOM transport contracts preserve native property values and exact serialized mutation provenance. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { reorderPaintStyleFixture } from './reorder-paint-style-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';

test('direct named properties share the actual serialized declaration and attribute owner', () => {
  const f = motionPaintFixture(),
    node = f.chips[0]!;
  node.style.opacity = '0.73';
  node.style.color = 'rgb(1, 2, 3)';
  assert.equal(node.style.getPropertyValue('opacity'), '0.73');
  assert.equal(node.getAttribute('style'), 'opacity: 0.73; color: rgb(1, 2, 3);');
  node.style.opacity = '';
  assert.equal(node.style.opacity, '');
  assert.equal(node.getAttribute('style'), 'color: rgb(1, 2, 3);');
});

test('serialized native copies trim declaration syntax and retain readonly font opacity and pointer values', () => {
  const style = reorderPaintStyleFixture();
  style.cssText = 'font: 14px serif; opacity: 0.73; pointer-events: none; border-radius: 9px;';
  assert.equal(style.font, '14px serif');
  assert.equal(style.opacity, '0.73');
  assert.equal(style['pointer-events'], 'none');
  assert.equal(style.borderRadius, '9px');
  assert.equal(style.getPropertyValue('border-radius'), '9px');
  style.setProperty('pointer-events', 'none', 'important');
  assert.equal(style.getPropertyValue('pointer-events'), 'none');
  assert.match(style.cssText, /pointer-events: none !important;/u);
});

test('native height setters emit their exact before lock and restore old values', () => {
  const f = motionPaintFixture(),
    observer = visibilityObserverFixture(f.view.dom.ownerDocument),
    track = observer.track(f.view.contentDOM);
  try {
    f.view.contentDOM.style.height = '20px';
    f.view.contentDOM.style.flexBasis = '';
    f.view.contentDOM.style.height = '';
    const records = track.take();
    assert.equal(records.length, 2);
    assert.deepEqual(
      records.map((record) => record.oldValue),
      [null, 'height: 20px;'],
    );
    assert.ok(
      records.every(
        (record) => record.target === f.view.contentDOM && record.attributeName === 'style',
      ),
    );
    assert.equal(f.view.contentDOM.getAttribute('style'), null);
  } finally {
    track.destroy();
  }
});
