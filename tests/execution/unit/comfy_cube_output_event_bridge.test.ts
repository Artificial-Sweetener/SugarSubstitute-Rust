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
/** Characterize prompt-only Cube output result capture at Comfy's event boundary. */

import {
  ComfyCubeOutputEventBridge,
  type ComfyExecutedEventListener,
} from '../../../frontend/comfyui/ui/cube/execution/ComfyCubeOutputEventBridge.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { CubeOutputExecutionStore } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionStore.js';

describe('ComfyCubeOutputEventBridge', () => {
  test('captures a prompt-only CubeOutput result that Comfy cannot map to a graph node', () => {
    const api = new FakeExecutionApi();
    const store = new CubeOutputExecutionStore();
    const bridge = new ComfyCubeOutputEventBridge({ api, store });
    const executionId = buildCubeOutputExecutionId('cube-instance', 0);
    const output = {
      images: [{ filename: 'cube-result.png', subfolder: '', type: 'temp' }],
    };

    bridge.install();
    api.emit({
      node: executionId,
      display_node: 'cube-instance:decode',
      output,
    });

    expect(store.read(executionId)).toBe(output);
  });

  test('ignores ordinary node results and detaches its exact listener on disposal', () => {
    const api = new FakeExecutionApi();
    const store = new CubeOutputExecutionStore();
    const bridge = new ComfyCubeOutputEventBridge({ api, store });
    bridge.install();

    api.emit({ node: 'cube-instance:decode', output: { images: [] } });
    expect(store.size).toBe(0);

    bridge.dispose();
    api.emit({
      node: buildCubeOutputExecutionId('cube-instance', 0),
      output: { images: [{ filename: 'late.png' }] },
    });
    expect(store.size).toBe(0);
    expect(api.listenerCount).toBe(0);
  });

  test('uses a synthetic display identity when the backend reports it there', () => {
    const api = new FakeExecutionApi();
    const store = new CubeOutputExecutionStore();
    const bridge = new ComfyCubeOutputEventBridge({ api, store });
    const executionId = buildCubeOutputExecutionId('cube-instance', 1);
    bridge.install();

    api.emit({
      node: 'backend-node',
      display_node: executionId,
      output: { text: ['result'] },
    });

    expect(store.read(executionId)).toEqual({ text: ['result'] });
  });
});

class FakeExecutionApi {
  #listeners = new Set<ComfyExecutedEventListener>();

  get listenerCount(): number {
    return this.#listeners.size;
  }

  addEventListener(name: 'executed', listener: ComfyExecutedEventListener): void {
    if (name === 'executed') this.#listeners.add(listener);
  }

  removeEventListener(name: 'executed', listener: ComfyExecutedEventListener): void {
    if (name === 'executed') this.#listeners.delete(listener);
  }

  emit(detail: Record<string, unknown>): void {
    for (const listener of this.#listeners) listener({ detail });
  }
}
