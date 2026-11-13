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
/** Verify Cube presentation observes only relevant renderer presence. */

import { jest } from '@jest/globals';
import { ComfyRendererPresenceObserver } from '../../../frontend/comfyui/ui/surface/ComfyRendererPresenceObserver.js';

describe('ComfyRendererPresenceObserver', () => {
  test('reports catalogued root insertion and removal while ignoring face mutations', async () => {
    document.body.replaceChildren();
    const changed = jest.fn();
    const observer = new ComfyRendererPresenceObserver({
      document,
      ownsNodeId: (nodeId) => nodeId === 'cube-1',
      onPresenceChange: changed,
    });
    const unrelated = document.createElement('div');
    unrelated.className = 'lg-node';
    unrelated.dataset.nodeId = 'ordinary-1';
    document.body.append(unrelated);
    await flushMutations();
    expect(changed).not.toHaveBeenCalled();

    const cubeRoot = document.createElement('div');
    cubeRoot.className = 'lg-node';
    cubeRoot.dataset.nodeId = 'cube-1';
    document.body.append(cubeRoot);
    await flushMutations();
    expect(changed).toHaveBeenCalledTimes(1);

    const faceChild = document.createElement('div');
    cubeRoot.append(faceChild);
    await flushMutations();
    expect(changed).toHaveBeenCalledTimes(1);

    cubeRoot.remove();
    await flushMutations();
    expect(changed).toHaveBeenCalledTimes(2);

    document.body.classList.add('litegraph');
    await flushMutations();
    expect(changed).toHaveBeenCalledTimes(3);
    observer.dispose();
  });
});

/** Flush one MutationObserver delivery turn. */
async function flushMutations(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
