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
/** An editor measures its text once per font, however often the scale it is shown at changes. */
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { retainTextSize } from '../../../../../../frontend/sugarprompt/editor/src/text-size-retention.ts';

/** A view whose document measures 20 by 8 by 16 units of text at scale one. */
function editor() {
  const style = {
    fontFamily: 'Inter',
    fontSize: '13px',
    fontWeight: '400',
    fontStyle: 'normal',
    fontStretch: '100%',
    letterSpacing: 'normal',
    lineHeight: '20px',
  };
  const fonts = new EventTarget();
  const state = { scaleX: 1, scaleY: 1, measurements: 0, visible: true };
  const docView = {
    measureTextSize() {
      state.measurements += 1;
      const shown = state.visible ? 1 : 0;
      return {
        lineHeight: 20 * state.scaleY * shown,
        charWidth: 8 * state.scaleX * shown,
        textHeight: 16 * state.scaleY * shown,
      };
    },
  };
  const contentDOM = {};
  const view = {
    docView,
    contentDOM,
    get scaleX() {
      return state.scaleX;
    },
    get scaleY() {
      return state.scaleY;
    },
    dom: {
      ownerDocument: {
        fonts,
        defaultView: {
          getComputedStyle: (element: unknown) => (element === contentDOM ? style : {}),
        },
      },
    },
  };
  return { view: view as unknown as EditorView, docView, state, style, fonts };
}

test('a change of scale alone answers from the last measurement, scaled', () => {
  const { view, docView, state } = editor();
  assert.equal(retainTextSize(view), true);
  assert.deepEqual(docView.measureTextSize(), { lineHeight: 20, charWidth: 8, textHeight: 16 });
  assert.equal(state.measurements, 1);
  for (const [scaleX, scaleY] of [
    [0.5, 0.5],
    [1.75, 1.75],
    [0.25, 0.5],
  ] as const) {
    state.scaleX = scaleX;
    state.scaleY = scaleY;
    assert.deepEqual(docView.measureTextSize(), {
      lineHeight: 20 * scaleY,
      charWidth: 8 * scaleX,
      textHeight: 16 * scaleY,
    });
  }
  assert.equal(state.measurements, 1, 'the document was probed once');
});

test('a changed font or a font that finished loading is measured again', () => {
  const { view, docView, state, style, fonts } = editor();
  retainTextSize(view);
  docView.measureTextSize();
  docView.measureTextSize();
  assert.equal(state.measurements, 1);
  style.fontSize = '15px';
  docView.measureTextSize();
  assert.equal(state.measurements, 2);
  style.lineHeight = '24px';
  docView.measureTextSize();
  assert.equal(state.measurements, 3);
  fonts.dispatchEvent(new Event('loadingdone'));
  docView.measureTextSize();
  docView.measureTextSize();
  assert.equal(state.measurements, 4);
});

test('a measurement taken while the editor has no size is never retained', () => {
  const { view, docView, state } = editor();
  retainTextSize(view);
  state.visible = false;
  assert.deepEqual(docView.measureTextSize(), { lineHeight: 0, charWidth: 0, textHeight: 0 });
  state.visible = true;
  assert.deepEqual(docView.measureTextSize(), { lineHeight: 20, charWidth: 8, textHeight: 16 });
  assert.equal(state.measurements, 2);
  docView.measureTextSize();
  assert.equal(state.measurements, 2);
});

test('a view without the measurement is left alone', () => {
  assert.equal(retainTextSize({ dom: {} } as unknown as EditorView), false);
});

test('the editor library in use still measures its text through the wrapped method', () => {
  // The method is internal to CodeMirror; an upgrade that renames it must fail here, not silently
  // bring the per-zoom-step probe back.
  const source = readFileSync('node_modules/@codemirror/view/dist/index.js', 'utf8');
  assert.match(source, /\n {4}measureTextSize\(\) \{/);
  assert.match(source, /view\.docView\.measureTextSize\(\)/);
  assert.match(source, /get docView\(\)|this\.docView = /);
});
