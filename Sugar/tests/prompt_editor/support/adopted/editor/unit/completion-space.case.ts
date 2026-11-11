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
/** Independent native-space operands prove this completion owner's gap clamp, without reproducing native placement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import {
  completionHeight,
  readCompletionHeight,
  applyCompletionHeight,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-space.ts';
import { CompletionNode } from './completion-dom-fixture.ts';

test('completion reserves six physical pixels and actual borders on either native side', () => {
  const space = { left: 0, right: 800, top: 10, bottom: 510 };
  const caret = { left: 90, right: 91, top: 110, bottom: 130 };
  assert.deepEqual(completionHeight(space, caret, false, 1, 2, 19), {
    outer: 374,
    inner: 372,
    visible: true,
  });
  assert.deepEqual(completionHeight(space, caret, true, 1, 2, 19), {
    outer: 94,
    inner: 92,
    visible: true,
  });
  assert.deepEqual(completionHeight(space, caret, true, 1.25, 2, 19), {
    outer: 75.2,
    inner: 73.2,
    visible: true,
  });
  assert.deepEqual(completionHeight(space, { ...caret, top: 13 }, true, 1, 2, 19), {
    outer: 0,
    inner: 0,
    visible: false,
  });
  assert.equal(completionHeight(space, caret, false, 0, 2, 19), null);
});

test('native popup vertical scale and unavailable caret keep finite visibility and recover without editor scale', () => {
  const native = new CompletionNode(),
    dom = native as unknown as HTMLElement;
  Object.defineProperty(native.ownerDocument.defaultView, 'getComputedStyle', {
    value: () => ({
      height: '80px',
      borderTopWidth: '1px',
      borderBottomWidth: '1px',
      paddingTop: '2px',
      paddingBottom: '6px',
    }),
  });
  Object.defineProperty(native, 'getBoundingClientRect', {
    value: () => ({ width: 300, height: 160 }),
  });
  Object.defineProperty(native.classList, 'contains', { value: () => false });
  let caret: {
    left: number;
    right: number;
    top: number;
    bottom: number;
  } | null = { left: 90, right: 91, top: 110, bottom: 130 };
  const view = {
    coordsAtPos: () => caret,
    defaultLineHeight: 19,
    scaleY: 7,
  } as unknown as EditorView;
  const space = { left: 0, right: 800, top: 10, bottom: 510 };
  const height = readCompletionHeight(view, dom, space, 0);
  assert.deepEqual(height, { outer: 187, inner: 177, visible: true });
  assert.equal(
    applyCompletionHeight(dom, height, () => true),
    'changed',
  );
  assert.equal(native.style.visibility, '');
  caret = null;
  assert.equal(readCompletionHeight(view, dom, space, 0), null);
  assert.equal(
    applyCompletionHeight(dom, null, () => true),
    'changed',
  );
  assert.equal(native.style.visibility, 'hidden');
  assert.equal(native.style['--sp-completion-outer-height'], '');
  assert.equal(
    applyCompletionHeight(dom, height, () => true),
    'changed',
  );
  assert.equal(native.style.visibility, '');
  assert.equal(
    applyCompletionHeight(dom, height, () => false),
    'retired',
  );
});
