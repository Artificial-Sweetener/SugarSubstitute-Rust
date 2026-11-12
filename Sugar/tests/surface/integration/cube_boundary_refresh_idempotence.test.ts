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
/** Verify settled Cube boundary refreshes preserve native DOM presentation identity. */
import { ComfyVueCubeBoundaryHost } from '../../../frontend/comfyui/ui/surface/ComfyVueCubeBoundaryHost.js';

test('unchanged boundary refreshes do not mutate slots or output leaders', () => {
  const body = document.createElement('div');
  const row = document.createElement('div');
  const input = document.createElement('div');
  input.className = 'lg-slot lg-slot--input';
  const output = document.createElement('div');
  output.className = 'lg-slot lg-slot--output';
  row.append(input, output);
  const title = document.createElement('header');
  title.dataset.cubePreviewOutputTitle = '';
  body.append(row, title);
  const host = new ComfyVueCubeBoundaryHost({
    body,
    node: { id: 'cube', pos: [0, 0], size: [500, 320], inputs: [{ type: 'IMAGE' }] },
  });
  const observer = new MutationObserver(() => undefined);
  observer.observe(body, { attributes: true, childList: true, subtree: true });
  try {
    host.reconcile();
    host.reconcile();
    expect(observer.takeRecords()).toHaveLength(0);

    output.style.setProperty('--sugarcube-boundary-position', '900px');
    observer.takeRecords();
    host.reconcile();
    expect(output.style.getPropertyValue('--sugarcube-boundary-position')).not.toBe('900px');
    expect(observer.takeRecords().length).toBeGreaterThan(0);
  } finally {
    observer.disconnect();
    host.dispose();
  }
});
