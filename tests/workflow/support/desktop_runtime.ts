/** @jest-environment node */
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
/** Compose the real production desktop runtime and canvas owners over fake Comfy and artifact boundaries. */
import { comfyDesktopHostBoundary } from './comfy_desktop_host.js';
import { desktopSessionFixture } from './desktop_session.js';
import { ComfyWorkflowExecutionRuntime } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import { ComfyWorkflowViewPreferences } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyWorkflowViewPreferences.js';
import { ComfyDesktopSessionRuntime } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyDesktopSessionRuntime.js';
import { WorkflowEventApi, WorkflowTestClock } from '../../execution/support/workflow_results.js';
import { CanvasViewRegistry } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasViewRegistry.js';
import { CanvasSourceInventory } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceInventory.js';
import { CanvasSessionCoordinator } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionCoordinator.js';
import { CanvasSelectionController } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSelectionController.js';
import { CanvasInspectionStore } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasInspectionStore.js';
import { CanvasSessionSnapshots } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSessionSnapshots.js';
import { decodeComfyWorkflowSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSourceDecoder.js';
import type {
  DesktopArtifactResult,
  DesktopOutputArtifacts,
} from '../../../frontend/comfyui/ui/execution/domain/DesktopOutputArtifacts.js';
import type { SettingsWorkspaceRoute } from '../../../frontend/comfyui/ui/settings/SettingsWorkspaceRoute.js';

/** Keep the behavior under test actual while controlling only host, clock, bytes and UUID generation. */
export async function desktopRuntimeFixture(settingsRoute?: SettingsWorkspaceRoute) {
  const shared = desktopSessionFixture();
  const host = comfyDesktopHostBoundary();
  const graph = { nodes: [{ id: 7, type: 'SaveImage', title: 'Final' }], extra: {} };
  await host.app.loadGraphData(
    graph,
    true,
    true,
    host.workflow.createNewTemporary('Initial', graph),
  );
  const errors: unknown[] = [];
  const api = Object.assign(new WorkflowEventApi(), {
    fetchApi: async () => {
      throw new Error('Unexpected history request.');
    },
  });
  let sequence = 500;
  const nextId = () => `00000000-0000-4000-8000-${(++sequence).toString(16).padStart(12, '0')}`;
  const execution = new ComfyWorkflowExecutionRuntime({
    app: host.app,
    api,
    clock: new WorkflowTestClock(),
    nextKey: nextId,
    logger: { error: (...args) => errors.push(args), warn: () => {} },
    previewUrls: { create: () => 'blob:unused', revoke: () => {} },
    previewVisibility: { read: () => false, subscribe: () => () => {} },
  });
  execution.install();
  const views = new CanvasViewRegistry(nextId);
  const inventory = new CanvasSourceInventory();
  const stopInventory = execution.sessions.subscribe((snapshot) => {
    if (snapshot.active)
      inventory.replace(
        snapshot.active,
        decodeComfyWorkflowSources(host.app.rootGraph.serialize()),
      );
  });
  const active = execution.sessions.read().active;
  if (!active) throw new Error('Missing active workflow.');
  inventory.replace(active, decodeComfyWorkflowSources(graph));
  const selection = new CanvasSelectionController(views, {
    latestOrder: (id) => execution.latestSubmissionOrder(id),
  });
  const inspections = new CanvasInspectionStore();
  const coordinator = new CanvasSessionCoordinator(
    execution.sessions,
    execution.results,
    inventory,
    views,
    selection,
    { subscribe: () => () => {} },
  );
  coordinator.install();
  const canvas = new CanvasSessionSnapshots(coordinator, views, selection, inspections);
  const presentation = new ComfyWorkflowViewPreferences(
    execution.sessions,
    () => null,
    () => true,
    { warn: () => {} },
  );
  presentation.setup();
  const recorded = shared.checkpoint.entries[0]?.outputs[0];
  if (!recorded) throw new Error('Missing artifact fixture.');
  let resolve: DesktopOutputArtifacts['resolve'] = async (output) => ({
    available: true,
    receipt: {
      file: output.file,
      digest: output.digest,
      bytes: output.bytes,
      width: output.width,
      height: output.height,
      media: output.media,
      mime: output.mime,
      metadataEmbedded: output.metadataEmbedded,
      animated: false,
    },
  });
  let receipt: DesktopOutputArtifacts['receipt'] = async (file) => ({
    available: true,
    receipt: {
      file: {
        storage: file.type,
        filename: file.filename,
        subfolder: file.subfolder,
        managedArtifact: null,
      },
      digest: recorded.digest,
      bytes: recorded.bytes,
      width: recorded.width,
      height: recorded.height,
      media: recorded.media,
      mime: recorded.mime,
      metadataEmbedded: false,
      animated: false,
    },
  });
  const artifacts: DesktopOutputArtifacts = {
    receipt: (...args) => receipt(...args),
    resolve: (...args) => resolve(...args),
  };
  const runtime = new ComfyDesktopSessionRuntime({
    app: host.app,
    scope: shared.checkpoint.scope,
    webview: shared.fence.webview,
    nextId,
    now: () => 1234,
    execution,
    canvas,
    presentation,
    artifacts,
    ...(settingsRoute ? { settingsRoute } : {}),
    versions: async () => ({ frontend: '1.47.11', sugarcubes: '0.15.3' }),
    report: (error) => errors.push(error),
  });
  runtime.install();
  return {
    ...shared,
    checkpoint: {
      ...shared.checkpoint,
      route: 'workflow' as const,
      entries: shared.checkpoint.entries.map((entry) => ({
        ...entry,
        document: { ...entry.document, graphJson: JSON.stringify(graph) },
      })),
    },
    host,
    api,
    execution,
    canvas,
    presentation,
    runtime,
    views,
    selection,
    inspections,
    coordinator,
    errors,
    nextId,
    setResolve: (value: DesktopOutputArtifacts['resolve']) => {
      resolve = value;
    },
    setReceipt: (value: DesktopArtifactResult | DesktopOutputArtifacts['receipt']) => {
      receipt = typeof value === 'function' ? value : async () => value;
    },
    dispose: () => {
      runtime.dispose();
      canvas.dispose();
      coordinator.dispose();
      presentation.dispose();
      stopInventory();
      execution.dispose();
      inventory.dispose();
      inspections.dispose();
      selection.dispose();
    },
  };
}
