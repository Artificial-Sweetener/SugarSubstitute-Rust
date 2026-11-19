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
/** Prove a file dragged over the tab bar is previewed as the tab it opens and lands where dropped. */
import { expect, jest, test } from '@jest/globals';
import { DraggedFiles } from '../../../frontend/comfyui/ui/core/file_drag/DraggedFiles.js';
import {
  fileTabTitle,
  WorkflowFileTabDrop,
} from '../../../frontend/comfyui/ui/surface/frame/WorkflowFileTabDrop.js';

function fixture() {
  const dragged = new DraggedFiles();
  const tabBar = {
    hit: jest.fn((_x: number, y: number): number | null => (y < 30 ? 1 : null)),
    preview: jest.fn(),
  };
  const placeNextOpened = jest.fn();
  const drop = new WorkflowFileTabDrop({ document, dragged, tabBar, placeNextOpened });
  drop.setup();
  return { dragged, tabBar, placeNextOpened, drop };
}

/** Dispatch one drag event that carries files, as a browser reports a drag from the desktop. */
function drag(type: string, y: number, init: { files?: boolean; related?: Element | null } = {}) {
  const event = new MouseEvent(type, { bubbles: true, clientX: 200, clientY: y });
  Object.defineProperty(event, 'dataTransfer', {
    value: { types: init.files === false ? ['text/plain'] : ['Files'] },
  });
  Object.defineProperty(event, 'relatedTarget', { value: init.related ?? null });
  document.body.dispatchEvent(event);
}

test('only files ComfyUI opens as a workflow get a tab title, without their extension', () => {
  expect(fileTabTitle('Portrait v2.json')).toBe('Portrait v2');
  expect(fileTabTitle('render.PNG')).toBe('render');
  expect(fileTabTitle('notes.txt')).toBeNull();
  expect(fileTabTitle('.json')).toBeNull();
});

test('the ghost tab shows once the host has named the file and the drag is over the tab bar', () => {
  const f = fixture();
  drag('dragover', 10);
  expect(f.tabBar.preview).not.toHaveBeenCalled();
  f.dragged.set(['Portrait.json', 'other.json']);
  expect(f.tabBar.preview).toHaveBeenLastCalledWith('Portrait', 1);
  drag('dragover', 200);
  expect(f.tabBar.preview).toHaveBeenLastCalledWith(null, 0);
  f.drop.dispose();
});

test('a drag that carries no files, or a file that opens no workflow, shows nothing', () => {
  const f = fixture();
  f.dragged.set(['Portrait.json']);
  drag('dragover', 10, { files: false });
  f.dragged.set(['notes.txt']);
  drag('dragover', 10);
  expect(f.tabBar.preview).not.toHaveBeenCalled();
  f.drop.dispose();
});

test('dropping on the tab bar asks for the opened workflow to take that place', () => {
  const f = fixture();
  f.dragged.set(['Portrait.json']);
  drag('dragover', 10);
  // The host may say the drag ended before the page sees its own drop.
  f.dragged.set(null);
  expect(f.tabBar.preview).toHaveBeenLastCalledWith('Portrait', 1);
  drag('drop', 10);
  expect(f.tabBar.preview).toHaveBeenLastCalledWith(null, 0);
  expect(f.placeNextOpened.mock.calls).toEqual([[1]]);
  f.drop.dispose();
});

test('a placement no workflow took is withdrawn when the person does anything else', () => {
  const f = fixture();
  f.dragged.set(['Portrait.json']);
  drag('dragover', 10);
  drag('drop', 10);
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(f.placeNextOpened.mock.calls).toEqual([[1], [null]]);
  f.drop.dispose();
});

test('a drop away from the tab bar, or a drag that leaves the window, places nothing', () => {
  const f = fixture();
  f.dragged.set(['Portrait.json']);
  drag('dragover', 200);
  drag('drop', 200);
  drag('dragover', 10);
  drag('dragleave', 10, { related: document.body });
  expect(f.tabBar.preview).toHaveBeenLastCalledWith('Portrait', 1);
  drag('dragleave', 10);
  expect(f.tabBar.preview).toHaveBeenLastCalledWith(null, 0);
  expect(f.placeNextOpened).not.toHaveBeenCalled();
  f.drop.dispose();
});
