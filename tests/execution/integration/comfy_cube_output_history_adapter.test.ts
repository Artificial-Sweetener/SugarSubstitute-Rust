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
/** Characterize Cube output reconciliation from Comfy's authoritative history. */

import { jest } from '@jest/globals';
import {
  ComfyCubeOutputHistoryAdapter,
  type ComfyExecutionSuccessListener,
} from '../../../frontend/comfyui/ui/cube/execution/ComfyCubeOutputHistoryAdapter.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';
import { CubeOutputExecutionStore } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionStore.js';

describe('ComfyCubeOutputHistoryAdapter', () => {
  test('publishes only validated changes to preview subscribers', () => {
    const executionId = buildCubeOutputExecutionId('cube-instance', 0);
    const store = new CubeOutputExecutionStore();
    const changed = jest.fn();
    const unsubscribe = store.subscribe(changed);

    expect(store.retain('ordinary-node', { text: ['ignored'] })).toBe(false);
    expect(changed).not.toHaveBeenCalled();
    expect(store.retain(executionId, { text: ['preview'] })).toBe(true);
    expect(changed).toHaveBeenCalledTimes(1);
    store.clear();
    expect(changed).toHaveBeenCalledTimes(2);
    store.clear();
    expect(changed).toHaveBeenCalledTimes(2);

    unsubscribe();
    store.retain(executionId, { text: ['later'] });
    expect(changed).toHaveBeenCalledTimes(2);
  });

  test('reconciles the completed prompt output Comfy cannot map to a graph node', async () => {
    const executionId = buildCubeOutputExecutionId('cube-instance', 0);
    const output = { images: [{ filename: 'history-result.png', type: 'temp' }] };
    const api = new FakeHistoryApi({
      '/history/prompt-1': {
        'prompt-1': { outputs: { [executionId]: output } },
      },
    });
    const store = new CubeOutputExecutionStore();
    const adapter = new ComfyCubeOutputHistoryAdapter({ api, store });

    expect(await adapter.reconcilePrompt('prompt-1')).toBe(1);
    expect(store.read(executionId)).toBe(output);
  });

  test('hydrates recent outputs oldest-to-newest so the latest result wins', async () => {
    const executionId = buildCubeOutputExecutionId('cube-instance', 0);
    const api = new FakeHistoryApi({
      '/history?max_items=50': {
        older: { outputs: { [executionId]: { text: ['old'] } } },
        newer: { outputs: { [executionId]: { text: ['new'] } } },
      },
    });
    const store = new CubeOutputExecutionStore();
    const adapter = new ComfyCubeOutputHistoryAdapter({ api, store });

    expect(await adapter.hydrateRecent()).toBe(2);
    expect(store.read(executionId)).toEqual({ text: ['new'] });
  });

  test('listens for completion and detaches its exact listener', async () => {
    const executionId = buildCubeOutputExecutionId('cube-instance', 0);
    const api = new FakeHistoryApi({
      '/history/prompt-2': {
        'prompt-2': { outputs: { [executionId]: { text: ['done'] } } },
      },
    });
    const store = new CubeOutputExecutionStore();
    const adapter = new ComfyCubeOutputHistoryAdapter({ api, store });
    adapter.install();

    api.emitSuccess({ prompt_id: 'prompt-2', timestamp: 1 });
    await api.flush();
    expect(store.read(executionId)).toEqual({ text: ['done'] });

    adapter.dispose();
    expect(api.listenerCount).toBe(0);
  });
});

class FakeHistoryApi {
  readonly #responses: Record<string, unknown>;
  readonly #listeners = new Set<ComfyExecutionSuccessListener>();
  readonly #pending: Promise<void>[] = [];

  constructor(responses: Record<string, unknown>) {
    this.#responses = responses;
  }

  get listenerCount(): number {
    return this.#listeners.size;
  }

  addEventListener(name: 'execution_success', listener: ComfyExecutionSuccessListener): void {
    if (name === 'execution_success') this.#listeners.add(listener);
  }

  removeEventListener(name: 'execution_success', listener: ComfyExecutionSuccessListener): void {
    if (name === 'execution_success') this.#listeners.delete(listener);
  }

  async fetchApi(path: string): Promise<{ ok: boolean; json(): Promise<unknown> }> {
    return {
      ok: Object.hasOwn(this.#responses, path),
      json: async () => this.#responses[path] ?? {},
    };
  }

  emitSuccess(detail: Record<string, unknown>): void {
    for (const listener of this.#listeners) {
      const result = listener({ detail });
      if (result instanceof Promise) this.#pending.push(result);
    }
  }

  async flush(): Promise<void> {
    await Promise.all(this.#pending);
  }
}
