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
/** Prove per-source editing is fenced, undoable and independent from node layout and widget values. */
import { jest } from '@jest/globals';
import { ComfySourceSavingDocument } from '../../../frontend/comfyui/ui/output_saving/ComfySourceSavingDocument.js';
import { readWorkflowSourceSavingPreferences } from '../../../frontend/comfyui/ui/output_saving/domain/SourceSavingPreferences.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

test('changes independent nested instances in one native history transaction without modifying their shared node', () => {
  const root: UnknownRecord = {
    extra: { foreign: { retained: true }, sugarcubes_output_saving: { workflow_name: 'Example' } },
  };
  const node: UnknownRecord = {
    type: 'SaveImage',
    title: 'Shared save',
    properties: {},
    widgets_values: ['Original prefix'],
    pos: [10, 20],
    size: [300, 400],
  };
  const original = JSON.stringify(node),
    previous = root.extra;
  const before = jest.fn(),
    after = jest.fn(),
    dirty = jest.fn();
  const scope = { id: 'workflow' };
  const editor = new ComfySourceSavingDocument({
    root: () => root,
    canvas: () => ({ emitBeforeChange: before, emitAfterChange: after, setDirty: dirty }),
    scope: () => scope,
    sources: () => [
      { path: 'a:save', node, label: 'First' },
      { path: 'b:save', node, label: 'Second' },
    ],
    capabilities: () =>
      new Map([['SaveImage', { classType: 'SaveImage', managed: true, kind: 'save' }]]),
  });
  editor.begin().commit(
    new Map([
      ['a:save', { enabled: false, label: '<literal>' }],
      ['b:save', { enabled: true, label: 'Second output' }],
    ]),
  );
  expect(before).toHaveBeenCalledTimes(1);
  expect(after).toHaveBeenCalledTimes(1);
  expect(readWorkflowSourceSavingPreferences(root).get('a:save')).toEqual({
    enabled: false,
    label: '<literal>',
  });
  expect(readWorkflowSourceSavingPreferences(root).get('b:save')?.enabled).toBe(true);
  expect(JSON.stringify(node)).toBe(original);
  expect(root.extra).toEqual(expect.objectContaining({ foreign: { retained: true } }));
  // Comfy owns snapshot restoration; restoring its pre-transaction metadata must restore both choices.
  root.extra = previous;
  expect(readWorkflowSourceSavingPreferences(root).size).toBe(0);
});

test('rejects a stale workflow or a replaced source without changing user data', () => {
  const root: UnknownRecord = { extra: {} };
  const canvas = { emitBeforeChange: jest.fn(), emitAfterChange: jest.fn(), setDirty: jest.fn() };
  const node: UnknownRecord = { type: 'SaveImage', properties: {} };
  let scope: object = {};
  const editor = new ComfySourceSavingDocument({
    root: () => root,
    canvas: () => canvas,
    scope: () => scope,
    sources: () => [{ path: 'save', node, label: 'Save' }],
    capabilities: () =>
      new Map([['SaveImage', { classType: 'SaveImage', managed: true, kind: 'save' }]]),
  });
  const session = editor.begin();
  scope = {};
  expect(session.current()).toBe(false);
  expect(() => session.commit(new Map([['save', { enabled: false, label: 'Other' }]]))).toThrow(
    'workflow changed',
  );
  expect(root.extra).toEqual({});
  expect(canvas.emitBeforeChange).not.toHaveBeenCalled();
});

test('keeps unsupported native saving authoritative and does not record unchanged edits', () => {
  const root: UnknownRecord = { extra: {} };
  const node: UnknownRecord = { type: 'CustomSaver' };
  const scope = {};
  const before = jest.fn();
  const editor = new ComfySourceSavingDocument({
    root: () => root,
    canvas: () => ({ emitBeforeChange: before, emitAfterChange: jest.fn(), setDirty: jest.fn() }),
    scope: () => scope,
    sources: () => [{ path: 'custom', node, label: 'Custom' }],
    capabilities: () =>
      new Map([['CustomSaver', { classType: 'CustomSaver', managed: false, kind: null }]]),
  });
  const session = editor.begin();
  session.commit(new Map([['custom', { enabled: true, label: null }]]));
  expect(before).not.toHaveBeenCalled();
  expect(() => session.commit(new Map([['custom', { enabled: false, label: null }]]))).toThrow(
    'native saving',
  );
  expect(root.extra).toEqual({});
});
