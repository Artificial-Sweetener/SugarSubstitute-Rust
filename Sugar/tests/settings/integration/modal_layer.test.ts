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
/** Prove SugarCubes dialogs allocate their own layer above visible host dialogs without moving the host. */
import { jest } from '@jest/globals';
import { mountModalLayer } from '../../../frontend/comfyui/ui/dialogs/ModalLayer.js';

test('clears the active host layer while ignoring hidden dialogs and its own content', () => {
  document.body.replaceChildren();
  const host = document.createElement('div');
  host.setAttribute('role', 'dialog');
  host.style.zIndex = '5305';
  document.body.append(host);
  // JSDOM does not lay out elements; provide the visible host's real-browser geometry boundary.
  const geometry = jest.spyOn(host, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    width: 900,
    height: 600,
    top: 0,
    left: 0,
    right: 900,
    bottom: 600,
    toJSON: () => ({}),
  });
  const hidden = document.createElement('div');
  hidden.setAttribute('role', 'dialog');
  hidden.style.zIndex = '9000';
  hidden.style.display = 'none';
  document.body.append(hidden);
  const overlay = document.createElement('div');
  const child = document.createElement('div');
  child.setAttribute('role', 'dialog');
  child.style.zIndex = '9500';
  overlay.append(child);
  mountModalLayer(document, overlay);
  expect(overlay.style.zIndex).toBe('5306');
  expect(host.parentElement).toBe(document.body);
  expect(overlay.parentElement).toBe(document.body);
  geometry.mockRestore();
  document.body.replaceChildren();
});
