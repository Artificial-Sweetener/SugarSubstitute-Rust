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
/** Prove drag preview retains section identity and ordinary runs. */
import { expect, test } from '@jest/globals';
import { previewMasonrySections } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryPreview.js';

test('moves Cube sections between their display slots while preserving every native reference', () => {
  const first = { id: 'cube:first', cube: {}, control: {} };
  const loose = { id: 'loose:10', cube: null, control: {} };
  const second = { id: 'cube:second', cube: {}, control: {} };
  const sections = [first, loose, second];
  const preview = previewMasonrySections(sections, ['cube:second', 'cube:first']);
  expect(preview).toEqual([second, loose, first]);
  expect(preview[0]).toBe(second);
  expect(preview[1]).toBe(loose);
  expect(sections).toEqual([first, loose, second]);
  expect(previewMasonrySections(sections, null)).toBe(sections);
});

test.each([
  { ids: ['cube:first'] },
  { ids: ['cube:first', 'cube:first'] },
  { ids: ['cube:first', 'foreign'] },
])('rejects stale, duplicated or foreign display order %j', ({ ids }) => {
  const sections = [
    { id: 'cube:first', cube: {} },
    { id: 'cube:second', cube: {} },
  ];
  expect(() => previewMasonrySections(sections, ids)).toThrow('exact Cube section permutation');
  expect(sections.map((section) => section.id)).toEqual(['cube:first', 'cube:second']);
});
