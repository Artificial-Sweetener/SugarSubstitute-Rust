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
/** Resolve negotiated output surfaces without creating or mutating an editor document. */
import { jest } from '@jest/globals';
import { WorkflowCanvasCoordinator } from '../../../frontend/comfyui/ui/execution/application/WorkflowCanvasCoordinator.js';
import { WorkflowGenerationEvents } from '../../../frontend/comfyui/ui/execution/application/WorkflowGenerationEvents.js';
import { documentCanvasAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/DocumentCanvasAdapter.js';
import { workflowResultsFixture } from '../support/workflow_results.js';
import type { CanvasLayerHost } from '../../../integrations/contracts/CanvasHost.js';
import type { CanvasDescriptor } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceHost } from '../../../integrations/contracts/CanvasSurface.js';

test('exposes only negotiated surfaces and rejects ambiguous registered peers', () => {
  const fixture = workflowResultsFixture();
  const report = jest.fn<(error: unknown) => void>();
  const events = new WorkflowGenerationEvents(report);
  const coordinator = new WorkflowCanvasCoordinator<Blob>(
    events,
    fixture.results,
    report,
    () => [],
  );
  const first: CanvasSurfaceHost = {
    execute: async () => ({ status: 'ready', message: '' }),
  };
  const second: CanvasSurfaceHost = {
    execute: async () => ({ status: 'unavailable', message: '' }),
  };
  let descriptor: CanvasDescriptor | null = null;
  let surface: CanvasSurfaceHost | null = first;
  const readContext = jest.fn<CanvasLayerHost['readContext']>();
  const insertImage = jest.fn<CanvasLayerHost['insertImage']>();
  const host: CanvasLayerHost = {
    get surface() {
      return surface;
    },
    readCanvas: () => descriptor,
    readContext,
    insertImage,
  };
  const adapter = documentCanvasAdapter<Blob>('embedded', host, { load: async () => null }, report);
  const remove = coordinator.register(adapter);
  expect(coordinator.surfaceHost()).toBeNull();
  descriptor = {
    id: 'native',
    label: 'Native',
    capabilities: { displayOutput: false, insertImageLayer: true, positionImageLayer: false },
  };
  expect(coordinator.surfaceHost('embedded')).toBeNull();
  descriptor = {
    ...descriptor,
    capabilities: { ...descriptor.capabilities, displayOutput: true, insertImageLayer: false },
  };
  expect(coordinator.surfaceHost()).toBe(first);
  expect(coordinator.surfaceHost('embedded')).toBe(first);
  expect(coordinator.surfaceHost('native')).toBeNull();
  surface = null;
  expect(coordinator.surfaceHost()).toBeNull();
  surface = first;
  const removeSecond = coordinator.register({
    id: 'second',
    capabilities: { output: { ordering: 'immediate', bind: () => null }, surface: second },
  });
  expect(coordinator.surfaceHost()).toBeNull();
  expect(coordinator.surfaceHost('second')).toBe(second);
  removeSecond();
  expect(coordinator.surfaceHost()).toBe(first);
  remove();
  expect(coordinator.surfaceHost()).toBeNull();
  coordinator.dispose();
  expect(readContext).not.toHaveBeenCalled();
  expect(insertImage).not.toHaveBeenCalled();
  expect(report).not.toHaveBeenCalled();
});
