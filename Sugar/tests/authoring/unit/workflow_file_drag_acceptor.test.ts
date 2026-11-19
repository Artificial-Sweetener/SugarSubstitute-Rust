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
/** Prove a file dragged from the desktop can be dropped on surfaces that cover Comfy's graph. */
import { expect, test } from '@jest/globals';
import { WorkflowFileDragAcceptor } from '../../../frontend/comfyui/ui/import/WorkflowFileDragAcceptor.js';

/** Drag `types` over `target` the way a browser reports a drag from the desktop or the page. */
function dragOver(target: Element, types: readonly string[]): DragEvent {
  const event = new Event('dragover', { bubbles: true, cancelable: true }) as DragEvent;
  Object.defineProperty(event, 'dataTransfer', { value: { types, dropEffect: 'none' } });
  target.dispatchEvent(event);
  return event;
}

test('a workflow file dragged over the masonry or the frame is accepted, so its drop reaches Comfy', () => {
  const masonry = document.createElement('section');
  masonry.className = 'sugarcubes-workflow-masonry';
  const frame = document.createElement('header');
  frame.className = 'sc-substitute-frame';
  document.body.append(masonry, frame);
  // Without the acceptor nothing answers, and a browser then refuses the drop outright.
  expect(dragOver(masonry, ['Files']).defaultPrevented).toBe(false);

  const acceptor = new WorkflowFileDragAcceptor(document);
  acceptor.setup();
  for (const surface of [masonry, frame]) {
    const event = dragOver(surface, ['Files']);
    expect(event.defaultPrevented).toBe(true);
    expect(event.dataTransfer?.dropEffect).toBe('copy');
  }
  // A drag that carries no file (text, a Cube from the picker) is none of its business.
  expect(dragOver(masonry, ['text/plain']).defaultPrevented).toBe(false);

  acceptor.dispose();
  expect(dragOver(masonry, ['Files']).defaultPrevented).toBe(false);
  document.body.replaceChildren();
});

test('an owner nearer the pointer keeps its own answer', () => {
  const node = document.createElement('div');
  document.body.append(node);
  let effect = '';
  node.addEventListener('dragover', (event) => {
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
  });
  const acceptor = new WorkflowFileDragAcceptor(document);
  acceptor.setup();
  const event = dragOver(node, ['Files']);
  effect = event.dataTransfer?.dropEffect ?? '';
  expect(event.defaultPrevented).toBe(true);
  expect(effect).toBe('move');
  acceptor.dispose();
  document.body.replaceChildren();
});
