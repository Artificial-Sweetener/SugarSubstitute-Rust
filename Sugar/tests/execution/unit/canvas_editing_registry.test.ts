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
/** Expose editing and workflow surfaces only on canvases that advertise them. */
import { jest } from '@jest/globals';
import { WorkflowCanvasCoordinator } from '../../../frontend/comfyui/ui/execution/application/WorkflowCanvasCoordinator.js';
import { WorkflowGenerationEvents } from '../../../frontend/comfyui/ui/execution/application/WorkflowGenerationEvents.js';
import { documentCanvasAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/DocumentCanvasAdapter.js';
import { workflowResultsFixture } from '../support/workflow_results.js';
import type { CanvasLayerHost } from '../../../integrations/contracts/CanvasHost.js';
import type { CanvasEditHost } from '../../../integrations/contracts/CanvasEditing.js';
import type { CanvasWorkflowHost } from '../../../integrations/contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasCapabilities,
  CanvasDescriptor,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const layer = { displayOutput: true, insertImageLayer: false, positionImageLayer: false };

/** Build editing and workflow hosts that never reach a transport. */
function hosts(canvasId: string): { editing: CanvasEditHost; workflow: CanvasWorkflowHost } {
  return {
    editing: {
      canvasId,
      execute: async () => ({ kind: 'failure', status: 'unsupported', message: 'Fixture.' }),
      changes: { subscribe: () => ({ ready: Promise.resolve('observing'), stop: () => {} }) },
    },
    workflow: {
      canvasId,
      execute: async () => ({ kind: 'failure', status: 'unsupported', message: 'Fixture.' }),
    },
  };
}

/** Compose the coordinator with no execution activity. */
function coordinator() {
  const report = jest.fn<(error: unknown) => void>();
  const events = new WorkflowGenerationEvents(report);
  return {
    report,
    canvases: new WorkflowCanvasCoordinator<Blob>(
      events,
      workflowResultsFixture().results,
      report,
      () => [],
    ),
  };
}

test('document adapters expose editing and workflow hosts only for advertised flags', () => {
  const { canvases, report } = coordinator();
  const native = hosts('native');
  let descriptor: CanvasDescriptor | null = null;
  const readContext = jest.fn<CanvasLayerHost['readContext']>();
  const insertImage = jest.fn<CanvasLayerHost['insertImage']>();
  const host: CanvasLayerHost = {
    ...native,
    readCanvas: () => descriptor,
    readContext,
    insertImage,
  };
  const adapter = documentCanvasAdapter<Blob>('embedded', host, { load: async () => null }, report);
  const remove = canvases.register(adapter);
  const advertise = (capabilities: CanvasCapabilities) => {
    descriptor = { id: 'native', label: 'Native', capabilities };
  };
  expect([adapter.capabilities.editing, adapter.capabilities.workflow]).toEqual([
    undefined,
    undefined,
  ]);
  for (const capabilities of [layer, { ...layer, geometry: true }]) {
    advertise(capabilities);
    expect(canvases.editHost()).toBeNull();
    expect(canvases.workflowHost()).toBeNull();
  }
  advertise({ ...layer, geometry: true, editing: true, workflowSurfaces: false });
  expect(canvases.editHost()).toBe(native.editing);
  expect(canvases.workflowHost()).toBeNull();
  advertise({ ...layer, geometry: false, editing: false, workflowSurfaces: true });
  expect(canvases.editHost()).toBeNull();
  expect(canvases.workflowHost('native')).toBe(native.workflow);
  expect(canvases.workflowHost('other')).toBeNull();
  remove();
  expect(canvases.workflowHost()).toBeNull();
  canvases.dispose();
  expect(readContext).not.toHaveBeenCalled();
  expect(insertImage).not.toHaveBeenCalled();
  expect(report).not.toHaveBeenCalled();
});

test('coordinator lookups refuse anonymous and ambiguous editing peers', () => {
  const { canvases, report } = coordinator();
  const output = { ordering: 'immediate', bind: () => null } as const;
  const first = hosts('editor-a');
  const second = hosts('editor-b');
  const anonymous = hosts('');
  canvases.register({ id: 'anonymous', capabilities: { output, ...anonymous } });
  expect(canvases.editHost()).toBeNull();
  const removeFirst = canvases.register({ id: 'first', capabilities: { output, ...first } });
  expect(canvases.editHost()).toBe(first.editing);
  expect(canvases.workflowHost()).toBe(first.workflow);
  const removeSecond = canvases.register({ id: 'second', capabilities: { output, ...second } });
  expect(canvases.editHost()).toBeNull();
  expect(canvases.workflowHost()).toBeNull();
  expect(canvases.editHost('editor-b')).toBe(second.editing);
  expect(canvases.workflowHost('editor-a')).toBe(first.workflow);
  removeFirst();
  expect(canvases.editHost()).toBe(second.editing);
  removeSecond();
  expect(canvases.workflowHost()).toBeNull();
  canvases.dispose();
  expect(report).not.toHaveBeenCalled();
});
