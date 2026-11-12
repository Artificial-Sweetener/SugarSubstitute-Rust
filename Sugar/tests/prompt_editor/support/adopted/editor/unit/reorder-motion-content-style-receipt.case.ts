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
/** Pinned DocView height-lock transport is admitted exactly, without exempting foreign content mutations. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareReorderVisibilityReceipt } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-visibility-receipt.ts';
import { setReorderMotionMask } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { currentReorderPaintLease } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';

/** One actual field/Widget owner uses independent literal native setter records, not manufactured receipt transitions. */
function fixture() {
  let observer: ReturnType<typeof visibilityObserverFixture> | null = null;
  const f = motionPresentationFixture(undefined, (document) => {
    observer = visibilityObserverFixture(document);
  });
  assert.ok(observer);
  const track = (observer as ReturnType<typeof visibilityObserverFixture>).track(f.view.contentDOM);
  const preview = f.state.field(reorderPreview).value!;
  const current = currentReorderPaintLease(f.view, preview)!;
  const next = { issue: {}, scope: {}, capture: preview.capture, ids: [1, 2] };
  return {
    f,
    track,
    current,
    next,
    destroy() {
      try {
        track.destroy();
      } finally {
        f.destroy();
      }
    },
  };
}

test('actual pinned local writes and exact content height lock restore are accepted', () => {
  const { f, track, current, next, destroy } = fixture();
  try {
    f.view.contentDOM.setAttribute('style', 'tab-size: 4;');
    track.take();
    const receipt = prepareReorderVisibilityReceipt(f.view, next, current);
    assert.ok(receipt);
    f.view.dispatch({ effects: setReorderMotionMask.of(next) });
    const records = track.take();
    assert.equal(records.length, 4);
    assert.deepEqual(
      records.map((record) => record.target === f.view.contentDOM),
      [false, false, true, true],
    );
    assert.equal(records[2]!.oldValue, 'tab-size: 4;');
    assert.equal(records[3]!.oldValue, 'tab-size: 4; height: 20px;');
    assert.equal(f.view.contentDOM.getAttribute('style'), 'tab-size: 4;');
    assert.equal(receipt(records), true);
  } finally {
    destroy();
  }
});

for (const fault of [
  'before',
  'after',
  'wrong-height',
  'intermediate',
  'class',
  'child',
  'flex-basis',
] as const)
  test(`exact content redraw rejects ${fault} without losing the current canonical lease`, () => {
    const { f, track, current, next, destroy } = fixture();
    try {
      const receipt = prepareReorderVisibilityReceipt(f.view, next, current);
      assert.ok(receipt);
      const content = f.view.contentDOM;
      if (fault === 'before') {
        content.style.height = '30px';
        content.style.height = '';
      }
      if (fault === 'wrong-height' || fault === 'intermediate') {
        const descriptor = Object.getOwnPropertyDescriptor(content.style, 'height')!;
        Object.defineProperty(content.style, 'height', {
          ...descriptor,
          set(value: string) {
            if (value && fault === 'intermediate') descriptor.set!.call(content.style, '30px');
            descriptor.set!.call(content.style, value && fault === 'wrong-height' ? '30px' : value);
          },
        });
      }
      f.view.dispatch({ effects: setReorderMotionMask.of(next) });
      if (fault === 'after') {
        content.style.height = '30px';
        content.style.height = '';
      }
      if (fault === 'class') content.setAttribute('class', content.className + ' foreign');
      if (fault === 'child') content.append(content.ownerDocument.createElement('span'));
      if (fault === 'flex-basis') content.style.flexBasis = '9px';
      assert.equal(receipt(track.take()), false);
      assert.equal(current(), true);
    } finally {
      destroy();
    }
  });

test('nonempty native pre-height refuses before publication', () => {
  const { f, current, next, destroy } = fixture();
  try {
    f.view.contentDOM.style.height = '30px';
    assert.equal(prepareReorderVisibilityReceipt(f.view, next, current), null);
    assert.equal(f.state.field(reorderPreview).mask, null);
  } finally {
    destroy();
  }
});
