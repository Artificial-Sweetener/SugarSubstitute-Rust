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
/** The actual projected widget admits only finite session pointer and native wheel routes; CM compatibility caret routes stay excluded. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { WidgetType } from '@codemirror/view';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { fixture, key } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';

test('real preview widget routes only the supported session pointer and wheel events', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    assert.equal(f.state.field(reorderPreview).native, true);
    f.owner.keydown(key('ArrowRight'));
    drain(f);
    assert.equal(f.state.field(reorderPreview).native, false);
    assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [1, 0, 2]);
    const widgets: WidgetType[] = [];
    f.state
      .field(reorderPreview)
      .decorations.between(0, f.state.doc.length, (_from, _to, value) => {
        if (value.spec.widget instanceof WidgetType) widgets.push(value.spec.widget);
      });
    assert.equal(widgets.length, 1);
    for (const type of [
      'wheel',
      'dragstart',
      'drop',
      'pointerdown',
      'pointermove',
      'pointerup',
      'pointercancel',
      'lostpointercapture',
    ]) {
      const event = new Event(type, { bubbles: true, cancelable: true });
      assert.equal(widgets[0]!.ignoreEvent(event), false);
      assert.equal(event.defaultPrevented, false);
    }
    for (const type of [
      'mousedown',
      'mousemove',
      'mouseup',
      'click',
      'dblclick',
      'keydown',
      'keyup',
      'beforeinput',
      'paste',
    ])
      assert.equal(widgets[0]!.ignoreEvent(new Event(type, { bubbles: true })), true);
  } finally {
    f.destroy();
  }
});
