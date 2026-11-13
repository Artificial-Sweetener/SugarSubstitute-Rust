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
/** Prove geometry capabilities remain independent of image delivery and reject ambiguous canvas targets. */
import { jest } from '@jest/globals';
import { WorkflowCanvasCoordinator } from '../../../frontend/comfyui/ui/execution/application/WorkflowCanvasCoordinator.js';
import { WorkflowGenerationEvents } from '../../../frontend/comfyui/ui/execution/application/WorkflowGenerationEvents.js';
import { workflowResultsFixture } from '../support/workflow_results.js';
import type { CanvasGeometryHost } from '../../../integrations/contracts/CanvasGeometry.js';

test('resolves geometry-only peers without adding editing to limited canvases or executing output', () => {
  const f = workflowResultsFixture();
  const report = jest.fn<(error: unknown) => void>();
  const events = new WorkflowGenerationEvents(report);
  const coordinator = new WorkflowCanvasCoordinator<Blob>(events, f.results, report, () => []);
  const first: CanvasGeometryHost = {
    canvasId: 'editor-a',
    execute: async () => ({ kind: 'failure', status: 'unsupported', message: 'Fixture.' }),
  };
  const second: CanvasGeometryHost = { ...first, canvasId: 'editor-b' };
  const output = { ordering: 'immediate', bind: () => null } as const;
  coordinator.register({ id: 'web', capabilities: { output } });
  expect(coordinator.geometryHost()).toBeNull();
  const removeFirst = coordinator.register({
    id: 'first',
    capabilities: { output, geometry: first },
  });
  expect(coordinator.geometryHost()).toBe(first);
  expect(coordinator.geometryHost('editor-a')).toBe(first);
  const removeSecond = coordinator.register({
    id: 'second',
    capabilities: { output, geometry: second },
  });
  expect(coordinator.geometryHost()).toBeNull();
  expect(coordinator.geometryHost('editor-b')).toBe(second);
  removeFirst();
  expect(coordinator.geometryHost()).toBe(second);
  removeSecond();
  expect(coordinator.geometryHost()).toBeNull();
  coordinator.dispose();
  expect(report).not.toHaveBeenCalled();
});
