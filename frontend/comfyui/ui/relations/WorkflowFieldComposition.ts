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
/** Compose native field appearance and relation policy around one submitted seed history. */
import { RelationHostLifecycle, type RelationHostOptions } from './RelationHostLifecycle.js';
import { AppearanceHost } from '../surface/appearance/AppearanceHost.js';
import { OwnedSurfaceAppearance } from '../surface/appearance/OwnedSurfaceAppearance.js';
import { WorkflowEditorNavigationHost } from '../surface/WorkflowEditorNavigationHost.js';
import { SubstituteFrameHost } from '../surface/frame/SubstituteFrameHost.js';
import type { WorkflowOutputActivity } from '../execution/application/WorkflowOutputActivity.js';
import type { WorkflowProgressStore } from '../execution/application/WorkflowProgressStore.js';
import type { CubeStackFrameControls } from '../surface/stack/CubeStackHost.js';
import { SettingsWorkspaceHost } from '../settings/SettingsWorkspaceHost.js';
import { EmbeddedPresentation } from '../core/embedded/EmbeddedPresentation.js';
import { startEmbeddedPanel } from '../core/embedded/EmbeddedPanelHost.js';
import { EmbeddedCanvasConnection } from '../core/embedded/EmbeddedCanvasConnection.js';
import type { MemoryBudgetRegistry } from '../core/memory/MemoryBudgetRegistry.js';
import { MemoryBudgetSettings } from '../settings/MemoryBudgetSettings.js';

export interface WorkflowFieldHostOptions
  extends Omit<RelationHostOptions, 'reportError' | 'appearance'> {
  reportRelationError(error: unknown): void;
  reportPresentationError(error: unknown): void;
  activity: WorkflowOutputActivity;
  progress: WorkflowProgressStore;
  stackControls(): CubeStackFrameControls;
  connected(): boolean;
  subscribeConnection(listener: (connected: boolean) => void): () => void;
  /** The page's cache registry, joined to a desktop host's memory budget when one exists. */
  memory: MemoryBudgetRegistry;
}

/** Share seed reuse across actual native field mounts and the override toolbar. */
export function createWorkflowFieldHost(options: WorkflowFieldHostOptions): {
  appearance: AppearanceHost;
  relations: RelationHostLifecycle;
  frame: SubstituteFrameHost;
  canvas: EmbeddedCanvasConnection;
  setupPresentation(): void;
} {
  const embedded = new EmbeddedPresentation();
  const canvas = new EmbeddedCanvasConnection();
  const relations: RelationHostLifecycle = new RelationHostLifecycle({
    ...options,
    reportError: options.reportRelationError,
    appearance: () => ownedAppearance,
  });
  const appearance: AppearanceHost = new AppearanceHost({
    app: options.app,
    document: options.document,
    seedReuse: (node, name) => relations.seedReuse.field(node, name),
    reportError: options.reportPresentationError,
    embedded,
  });
  const ownedAppearance = new OwnedSurfaceAppearance(options.document, appearance.state);
  const frame = new SubstituteFrameHost({
    app: options.app,
    document: options.document,
    palette: appearance.palette,
    sessions: options.sessions,
    activity: options.activity,
    progress: options.progress,
    stackControls: options.stackControls,
    openOverrides: (trigger) => relations.openOverrideMenu(trigger),
    setOverridesInline: (inline) => relations.setOverridesInline(inline),
    report: options.reportPresentationError,
  });
  const editorNavigation = new WorkflowEditorNavigationHost({
    app: options.app,
    document: options.document,
    appearance: appearance.state,
    suspendOverrides: (suspended) => relations.setEditorActive(suspended),
    report: options.reportPresentationError,
  });
  const settingsWorkspace = new SettingsWorkspaceHost({
    app: options.app,
    document: options.document,
    mode: frame.mode,
    palette: appearance.palette,
    report: options.reportPresentationError,
  });
  /** Mount field appearance and its frame/navigation presentation after native workspace setup. */
  function setupPresentation(): void {
    appearance.setup();
    frame.setup();
    relations.setupOverridePresentation(embedded);
    editorNavigation.setup();
    settingsWorkspace.setup();
    const windowRef = options.document.defaultView;
    if (windowRef)
      startEmbeddedPanel({
        window: windowRef,
        presentation: embedded,
        canvas,
        memory: {
          registry: options.memory,
          preferences: () => new MemoryBudgetSettings(options.app),
        },
        connected: options.connected,
        subscribeConnection: options.subscribeConnection,
        report: options.reportPresentationError,
      });
  }
  return { appearance, relations, frame, canvas, setupPresentation };
}
