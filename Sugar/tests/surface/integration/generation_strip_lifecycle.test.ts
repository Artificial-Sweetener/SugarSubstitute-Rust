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
/** Prove scoped native control observation and reversible renderer-independent strip mounting. */
import { jest } from '@jest/globals';
import { GenerationStripHost } from '../../../frontend/comfyui/ui/surface/frame/GenerationStripHost.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

/** Mount the stable Comfy actionbar contract shared by Nodes 1 and Nodes 2. */
function fixture() {
  const native = document.createElement('div');
  native.className = 'actionbar-container';
  const group = document.createElement('div');
  group.className = 'queue-button-group';
  const run = document.createElement('button');
  group.append(run);
  const cancel = document.createElement('button');
  cancel.setAttribute('aria-label', 'Cancel current run');
  cancel.disabled = true;
  const batch = document.createElement('div');
  batch.className = 'batch-count';
  native.append(group, cancel, batch);
  const frame = document.createElement('header');
  frame.className = 'sc-substitute-frame';
  document.body.append(native, frame);
  const changes = new Set<() => void>();
  const queueSettings = {
    batchCount: 1,
    mode: 'disabled',
    $subscribe: (listener: () => void) => {
      changes.add(listener);
      return () => changes.delete(listener);
    },
  };
  const execute = jest.fn<(id: string) => Promise<void>>().mockResolvedValue(undefined);
  const report = jest.fn<(error: unknown) => void>();
  const host = new GenerationStripHost(
    document,
    frame,
    { extensionManager: { queueSettings, command: { execute } } },
    report,
  );
  return { host, native, frame, cancel, queueSettings, changes, execute, report };
}

test('native execution updates and store edits preserve strip controls and focus', async () => {
  const host = fixture();
  const strip = host.frame.querySelector('.sc-generation-strip');
  const input = host.frame.querySelector('input')!;
  input.focus();
  input.value = '11';
  host.cancel.disabled = false;
  await Promise.resolve();
  expect(host.frame.querySelector('.sc-generation-strip')).toBe(strip);
  expect(input.value).toBe('11');
  expect(document.activeElement).toBe(input);
  expect(host.frame.querySelector<HTMLButtonElement>('[data-action="skip"]')!.disabled).toBe(false);
  host.queueSettings.batchCount = 3;
  for (const changed of host.changes) changed();
  expect(input.value).toBe('3');
  host.host.dispose();
  expect(host.changes.size).toBe(0);
  expect(host.native.querySelector('[data-sc-generation-native]')).toBeNull();
  expect(host.frame.querySelector('.sc-generation-strip')).toBeNull();
});

test('unrelated popup and graph mutations cause zero strip refreshes', async () => {
  const host = fixture();
  const input = host.frame.querySelector('input')!;
  const spy = jest.spyOn(input, 'setAttribute');
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  popup.textContent = '<literal popup>';
  const graph = document.createElement('canvas');
  document.body.append(graph);
  await Promise.resolve();
  expect(spy).not.toHaveBeenCalled();
  host.host.dispose();
});

test('replacement native actionbar restores retired surfaces and binds the new execution state', async () => {
  const host = fixture();
  const replacement = host.native.cloneNode(true);
  if (!(replacement instanceof HTMLElement)) throw new Error('invalid fixture');
  const cancel = replacement.querySelector<HTMLButtonElement>('[aria-label="Cancel current run"]')!;
  cancel.disabled = false;
  host.native.replaceWith(replacement);
  await Promise.resolve();
  expect(host.native.querySelector('[data-sc-generation-native]')).toBeNull();
  expect(host.frame.querySelector<HTMLButtonElement>('[data-action="cancel"]')!.disabled).toBe(
    false,
  );
  host.host.dispose();
  expect(replacement.querySelector('[data-sc-generation-native]')).toBeNull();
});
