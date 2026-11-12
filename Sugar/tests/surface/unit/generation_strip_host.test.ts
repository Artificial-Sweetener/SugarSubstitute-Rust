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
/** Safeguard native generation commands and settings before replacing their presentation. */
import { jest } from '@jest/globals';
import { ComfyGenerationAdapter } from '../../../frontend/comfyui/ui/surface/frame/ComfyGenerationAdapter.js';

afterEach(() => document.body.replaceChildren());

/** Mount the host-owned run controls without depending on a Nodes renderer. */
function fixture() {
  const native = document.createElement('div');
  native.className = 'actionbar-container';
  const run = document.createElement('button');
  run.textContent = 'Run';
  const group = document.createElement('div');
  group.className = 'queue-button-group';
  group.append(run);
  const cancel = document.createElement('button');
  cancel.setAttribute('aria-label', 'Cancel current run');
  cancel.disabled = true;
  const batch = document.createElement('div');
  batch.className = 'batch-count';
  native.append(batch, group, cancel);
  document.body.append(native);
  const execute = jest.fn<(id: string) => Promise<void>>().mockResolvedValue(undefined);
  const queueSettings = { batchCount: 1, mode: 'disabled', $subscribe: () => () => undefined };
  const app = { extensionManager: { queueSettings, command: { execute } } };
  return { native, run, group, cancel, batch, execute, queueSettings, app };
}

test('generation uses the native batch setting and canonical host command', async () => {
  const host = fixture();
  const adapter = new ComfyGenerationAdapter(document, host.app);
  adapter.setBatch(3);
  expect(host.queueSettings.batchCount).toBe(3);
  await adapter.generate();
  expect(host.execute.mock.calls).toEqual([['Comfy.QueuePrompt']]);
  expect(adapter.read().batch).toBe(3);
});

test('skip interrupts only; cancel stops auto queue and clears pending before interrupting', async () => {
  const host = fixture();
  host.cancel.disabled = false;
  host.queueSettings.mode = 'instant';
  const adapter = new ComfyGenerationAdapter(document, host.app);
  await adapter.skip();
  expect(host.execute.mock.calls).toEqual([['Comfy.Interrupt']]);
  expect(host.queueSettings.mode).toBe('instant');
  host.execute.mockClear();
  await adapter.cancel();
  expect(host.queueSettings.mode).toBe('disabled');
  expect(host.execute.mock.calls).toEqual([['Comfy.ClearPendingTasks'], ['Comfy.Interrupt']]);
});

test('missing host controls fail closed and native visibility is restored on disposal', () => {
  const host = fixture();
  const adapter = new ComfyGenerationAdapter(document, host.app);
  adapter.read();
  expect(host.batch.hasAttribute('data-sc-generation-native')).toBe(true);
  expect(host.group.hasAttribute('data-sc-generation-native')).toBe(true);
  expect(adapter.read().skipEnabled).toBe(false);
  adapter.dispose();
  expect(host.native.querySelector('[data-sc-generation-native]')).toBeNull();
  const absent = new ComfyGenerationAdapter(document, {});
  expect(absent.read().generateEnabled).toBe(false);
  expect(() => absent.setBatch(2)).toThrow();
});

test('batch validation clamps finite counts and rejects invalid values', () => {
  const host = fixture();
  const adapter = new ComfyGenerationAdapter(document, host.app);
  adapter.setBatch(2000);
  expect(host.queueSettings.batchCount).toBe(999);
  adapter.setBatch(0);
  expect(host.queueSettings.batchCount).toBe(1);
  expect(() => adapter.setBatch(Number.NaN)).toThrow();
});

test('cancel fences already admitted batch submissions before clearing and interrupts once', async () => {
  const host = fixture();
  let release: () => void = () => undefined;
  const admission = new Promise<void>((resolve) => {
    release = resolve;
  });
  host.execute.mockImplementation(async (id) => {
    if (id === 'Comfy.QueuePrompt') await admission;
  });
  const adapter = new ComfyGenerationAdapter(document, host.app);
  const submitting = adapter.generate();
  expect(adapter.read().cancelEnabled).toBe(true);
  const canceling = adapter.cancel();
  expect(adapter.read().generateEnabled).toBe(false);
  await adapter.generate();
  expect(host.execute.mock.calls).toEqual([['Comfy.QueuePrompt']]);
  release();
  await Promise.all([submitting, canceling]);
  expect(host.execute.mock.calls).toEqual([
    ['Comfy.QueuePrompt'],
    ['Comfy.ClearPendingTasks'],
    ['Comfy.Interrupt'],
  ]);
});

test('host action failures propagate and release the admission barrier', async () => {
  const host = fixture();
  const failure = new Error('submission rejected');
  host.execute.mockRejectedValueOnce(failure);
  const adapter = new ComfyGenerationAdapter(document, host.app);
  await expect(adapter.generate()).rejects.toBe(failure);
  expect(adapter.read().cancelEnabled).toBe(false);
  host.cancel.disabled = false;
  host.execute.mockRejectedValueOnce(failure);
  await expect(adapter.cancel()).rejects.toBe(failure);
  expect(adapter.read().generateEnabled).toBe(true);
});
