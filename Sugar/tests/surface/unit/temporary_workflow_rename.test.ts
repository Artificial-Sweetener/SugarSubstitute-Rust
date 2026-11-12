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
/** Prove draft rename preserves host identity, extension, cancellation and collision guards. */
import { expect, jest, test } from '@jest/globals';
import {
  ComfyWorkflowRename,
  type WorkflowRenamePrompt,
} from '../../../frontend/comfyui/ui/surface/frame/ComfyWorkflowRename.js';

/** Model the host store boundary without emulating browser or file persistence. */
function fixture(prompt: WorkflowRenamePrompt) {
  const workflow = {
    path: 'workflows/Old.json',
    filename: 'Old',
    fullFilename: 'Old.json',
    isTemporary: true,
    isPersisted: false,
  };
  const open = [workflow];
  const renameWorkflow = jest.fn(async (target: typeof workflow, path: string) => {
    target.path = path;
  });
  const store = {
    isBusy: false,
    workflows: [workflow],
    renameWorkflow,
    getWorkflowByPath: (path: string) => store.workflows.find((item) => item.path === path),
  };
  return { workflow, open, store, rename: new ComfyWorkflowRename(store, () => open, prompt) };
}

test('rename delegates the same draft to the host store without duplicating its extension', async () => {
  const prompt = jest.fn<WorkflowRenamePrompt>(async () => 'New.json');
  const f = fixture(prompt);
  expect(f.rename.available(f.workflow)).toBe(true);
  await f.rename.rename(f.workflow);
  expect(prompt).toHaveBeenCalledWith('Old', expect.any(Function));
  expect(f.store.renameWorkflow).toHaveBeenCalledWith(f.workflow, 'workflows/New.json');
  expect(f.open[0]).toBe(f.workflow);
});

test.each([null, 'Old'])('cancel or unchanged name %s preserves the draft', async (value) => {
  const f = fixture(async () => value);
  await f.rename.rename(f.workflow);
  expect(f.store.renameWorkflow).not.toHaveBeenCalled();
});

test('invalid names and collisions are validated before accepting the prompt', async () => {
  const f = fixture(async (_name, validate) => {
    expect(validate('')).toMatch(/Enter/);
    expect(validate('../bad')).toMatch(/filename/);
    expect(validate('OTHER')).toMatch(/already/);
    return 'Other';
  });
  f.store.workflows.push({ ...f.workflow, path: 'workflows/Other.json' });
  await expect(f.rename.rename(f.workflow)).rejects.toThrow('already');
  expect(f.store.renameWorkflow).not.toHaveBeenCalled();
});

test('closing a workflow while the prompt is open prevents a stale rename', async () => {
  const f = fixture(async () => {
    f.open.length = 0;
    return 'New';
  });
  await expect(f.rename.rename(f.workflow)).rejects.toThrow('changed');
  expect(f.store.renameWorkflow).not.toHaveBeenCalled();
});

test('persisted files and busy stores retain their native availability policy', () => {
  const f = fixture(async () => null);
  f.workflow.isPersisted = true;
  expect(f.rename.available(f.workflow)).toBe(false);
  f.workflow.isPersisted = false;
  f.store.isBusy = true;
  expect(f.rename.available(f.workflow)).toBe(false);
});
