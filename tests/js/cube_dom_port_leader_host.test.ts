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
/** Verify DOM output leaders join rendered labels to exact Cube socket centers. */

import { CubeDomPortLeaderHost } from '../../frontend/comfyui/ui/surface/CubeDomPortLeaderHost.js';

describe('CubeDomPortLeaderHost', () => {
  test('starts after rendered label text and terminates at the boundary socket', () => {
    const body = document.createElement('div');
    setRect(body, { left: 100, top: 50, width: 600, height: 400 });
    Object.defineProperty(body, 'offsetWidth', { configurable: true, value: 600 });
    const title = document.createElement('header');
    setRect(title, { left: 140, top: 90, width: 500, height: 20 });
    const label = document.createElement('span');
    label.dataset.cubePreviewOutputLabel = '';
    label.textContent = 'output.image';
    setRect(label, { left: 140, top: 90, width: 100, height: 20 });
    title.append(label);
    body.append(title);
    const host = new CubeDomPortLeaderHost(body);

    host.render([{ index: 0, title, portY: 170, socketRadius: 6 }]);

    expect(
      body.querySelector<SVGPathElement>('[data-sugarcube-output-leader="0"]')?.getAttribute('d'),
    ).toBe('M 146.00 50.00 H 570.00 V 170.00 H 594.00');
    host.dispose();
  });
});

/** Give one JSDOM element finite viewport geometry. */
function setRect(
  element: HTMLElement,
  rect: { left: number; top: number; width: number; height: number },
): void {
  element.getBoundingClientRect = () =>
    ({
      x: rect.left,
      y: rect.top,
      left: rect.left,
      top: rect.top,
      right: rect.left + rect.width,
      bottom: rect.top + rect.height,
      width: rect.width,
      height: rect.height,
      toJSON: () => ({}),
    }) as DOMRect;
}
