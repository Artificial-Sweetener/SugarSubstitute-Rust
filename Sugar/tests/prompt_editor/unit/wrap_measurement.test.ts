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
/** Prove wrapped native fragments count nested widget geometry once without losing row widths. */
import { jest } from '@jest/globals';
import {
  measurePromptWrapGroup,
  renderedPromptWidth,
} from '../../../frontend/sugarprompt/editor/src/prompt-wrap-measurement.js';

test('merge nested glyphs and superscript rectangles within their rendered row', () => {
  expect(
    renderedPromptWidth([
      { left: 0, right: 80, top: 0, bottom: 19 },
      { left: 0, right: 60, top: 2, bottom: 16 },
      { left: 60, right: 80, top: 0, bottom: 10 },
      { left: 80, right: 90, top: 2, bottom: 16 },
    ]),
  ).toBe(90);
});

test('sum soft-wrapped row fragments and ignore empty native anchors', () => {
  expect(
    renderedPromptWidth([
      { left: 20, right: 80, top: 0, bottom: 19 },
      { left: 0, right: 40, top: 19, bottom: 38 },
      { left: 10, right: 30, top: 20, bottom: 35 },
      { left: 0, right: 0, top: 19, bottom: 38 },
    ]),
  ).toBe(100);
});

test('reuse a native measurement range without allocating or retaining editable endpoints', () => {
  const contentDOM = document.createElement('div');
  const text = document.createTextNode('native text');
  contentDOM.append(text);
  document.body.append(contentDOM);
  const range = document.createRange();
  const rectangles = jest.fn(() => [{ left: 0, right: 60, top: 0, bottom: 20 }]);
  Object.defineProperty(range, 'getClientRects', { value: rectangles });
  const geometry = { contentDOM, domAtPos: (offset: number) => ({ node: text, offset }) };
  const selection = document.getSelection();
  selection?.collapse(text, 3);
  const allocate = jest.spyOn(document, 'createRange');
  try {
    for (let index = 0; index < 100; index++)
      expect(measurePromptWrapGroup(geometry, { from: 0, to: 6, kind: 'word' }, 0.75, range)).toBe(
        80,
      );
    expect(allocate).not.toHaveBeenCalled();
    expect(range.startContainer).toBe(contentDOM);
    expect(range.collapsed).toBe(true);
    expect(selection?.anchorNode).toBe(text);
    expect(selection?.anchorOffset).toBe(3);
    rectangles.mockImplementationOnce(() => {
      throw new Error('layout failure');
    });
    expect(() =>
      measurePromptWrapGroup(geometry, { from: 0, to: 6, kind: 'word' }, 1, range),
    ).toThrow('layout failure');
    expect(range.startContainer).toBe(contentDOM);
    expect(range.collapsed).toBe(true);
  } finally {
    allocate.mockRestore();
    selection?.removeAllRanges();
    contentDOM.remove();
  }
});
