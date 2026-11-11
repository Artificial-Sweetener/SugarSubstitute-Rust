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
/** Prove workflow names come from captured ownership without mutating serialized graphs or caller metadata. */
import { attachSavingRunContext } from '../../../frontend/comfyui/ui/output_saving/ComfySavingRunContext.js';
import { freezeWorkflowSubmissionContext } from '../../../frontend/comfyui/ui/execution/domain/WorkflowSubmission.js';

test('attaches the captured workflow filename independently of later active-tab state', () => {
  const workflow = { nodes: [], extra: {} };
  const payload = {
    workflow,
    output: {},
    extra_data: {
      private_context: 'retained',
      sugarcubes_output_saving: { another_field: 'retained' },
    },
  };
  const captured = freezeWorkflowSubmissionContext({
    session: {
      sessionId: 'workflow:a',
      title: 'My workflow.JSON',
      path: 'workflows/My workflow.JSON',
      generation: 1,
    },
    sources: [],
  });
  const result = attachSavingRunContext(payload, captured);
  expect(result).toEqual({
    ...payload,
    workflow: { nodes: [], extra: { sugarcubes_output_saving: { workflow_name: 'My workflow' } } },
    extra_data: {
      ...payload.extra_data,
      sugarcubes_output_saving: { another_field: 'retained', workflow_name: 'My workflow' },
    },
  });
  expect(payload.extra_data.sugarcubes_output_saving).toEqual({ another_field: 'retained' });
  expect(workflow).toEqual({ nodes: [], extra: {} });
});

test('unknown construction ownership is retained without guessing a currently active workflow', () => {
  const payload = { workflow: {}, output: {} };
  expect(attachSavingRunContext(payload, null)).toBe(payload);
});
