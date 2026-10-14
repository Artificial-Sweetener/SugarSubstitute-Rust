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
/** Safeguard canonical host actions and reversible frame preferences before replacing visuals. */
import { jest } from '@jest/globals';
import {
  SubstituteFrameMode,
  SUBSTITUTE_FRAME_SETTING,
} from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameMode.js';
import { ComfyFrameWorkflowAdapter } from '../../../frontend/comfyui/ui/surface/frame/ComfyFrameWorkflowAdapter.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

test('frame mode defaults off, changes independently, and registers one preference', () => {
  const mode = new SubstituteFrameMode();
  let setting: { id: string; defaultValue: boolean; onChange(value: unknown): void } | null = null;
  mode.setup({
    ui: {
      settings: {
        addSetting: (value: typeof setting) => {
          setting = value;
        },
        getSettingValue: () => false,
      },
    },
  });
  expect(mode.enabled).toBe(false);
  expect(setting!.id).toBe(SUBSTITUTE_FRAME_SETTING);
  expect(setting!.defaultValue).toBe(false);
  let changes = 0;
  mode.subscribe(() => changes++);
  mode.setEnabled(true);
  mode.setEnabled(true);
  expect(changes).toBe(1);
  setting!.onChange(false);
  expect(mode.enabled).toBe(false);
});

test('activation and inactive close invoke the existing native handlers without changing the store', () => {
  const id = workflowSessionId('original');
  const sessions = [{ sessionId: id, title: '<literal>', path: 'original', generation: 0 }];
  const workflow = { path: 'original', isModified: true };
  const store = { openWorkflows: [workflow], activeWorkflow: null };
  const native = document.createElement('div');
  const button = document.createElement('button');
  const tab = document.createElement('div');
  tab.className = 'workflow-tab';
  const label = document.createElement('span');
  label.className = 'workflow-label';
  label.textContent = sessions[0]!.title;
  tab.append(label);
  button.append(tab);
  native.append(button);
  let activated = 0;
  let closed = 0;
  button.addEventListener('click', (event) => {
    if (event.button === 0) activated++;
  });
  tab.addEventListener('mouseup', (event) => {
    if (event.button === 1) closed++;
  });
  const adapter = new ComfyFrameWorkflowAdapter({ extensionManager: { workflow: store } }, native);
  expect(adapter.read(sessions)[0]?.title).toBe('<literal>');
  adapter.activate(id, sessions);
  adapter.close(id, sessions);
  expect(activated).toBe(1);
  expect(closed).toBe(1);
  expect(store.activeWorkflow).toBe(null);
  native.replaceChildren();
  expect(() => adapter.close(id, sessions)).toThrow('action surface changed');
});

test('command delegation preserves asynchronous failures for the frame error boundary', async () => {
  const execute = jest.fn(async (id: string) => {
    if (id === 'Comfy.SaveWorkflow') throw new Error('save failed');
  });
  const adapter = new ComfyFrameWorkflowAdapter(
    { extensionManager: { workflow: { openWorkflows: [] }, command: { execute } } },
    document.createElement('div'),
  );
  await adapter.command('Comfy.ShowSettingsDialog');
  expect(execute).toHaveBeenCalledWith('Comfy.ShowSettingsDialog');
  await expect(adapter.command('Comfy.SaveWorkflow')).rejects.toThrow('save failed');
});
