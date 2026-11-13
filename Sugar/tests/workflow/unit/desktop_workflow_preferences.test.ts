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
/** Qualify inactive/temporary occurrence preferences and native import without premature browser writes. */
import { WorkflowViewPreferences } from '../../../frontend/comfyui/ui/workflow/application/WorkflowViewPreferences.js';

test('inactive browser history and temporary live choices capture without activating or writing', () => {
  const writes: unknown[] = [];
  const views = new WorkflowViewPreferences(
    [{ path: 'workflows/a.json', canvas: false, masonry: true, editorWidth: 444 }],
    (value) => writes.push(value),
  );
  const a = { id: 'a', path: 'workflows/a.json' };
  const temp = { id: 'temp', path: null };
  expect(views.snapshotFor(a, () => true)).toEqual({
    canvas: false,
    masonry: true,
    editorWidth: 444,
  });
  views.restore(temp, { canvas: true, masonry: true, editorWidth: 650 });
  views.activate(temp, () => false);
  expect(views.snapshot).toEqual({ canvas: true, masonry: true, editorWidth: 650 });
  expect(writes).toEqual([]);
  views.activate(a, () => true);
  expect(views.snapshot.editorWidth).toBe(444);
  expect(writes).toEqual([]);
});
