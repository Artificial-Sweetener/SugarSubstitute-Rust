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
import { HostAccent } from '../core/embedded/HostAccent.js';
import { HostDescriptionState } from '../core/embedded/HostDescriptionState.js';
import { UpdateSourceSlot } from '../core/updates/UpdateSource.js';
import { DraggedFiles } from '../core/file_drag/DraggedFiles.js';
import { BackendOutageNotice } from '../core/backend/BackendOutageNotice.js';
import { HostUpdateReport } from '../core/update_report/HostUpdateReportLink.js';
import { DeclaredHost } from '../core/embedded/PanelRuntime.js';
import { startEmbeddedPanel } from '../core/embedded/EmbeddedPanelHost.js';
import type { HostLine } from '../core/session_notice/HostSessionNoticeLink.js';
import { unsavedWorkWords } from '../core/session_notice/UnsavedWorkNotice.js';
import { EmbeddedCanvasConnection } from '../core/embedded/EmbeddedCanvasConnection.js';
import type { MemoryBudgetRegistry } from '../core/memory/MemoryBudgetRegistry.js';
import { MemoryBudgetSettings } from '../settings/MemoryBudgetSettings.js';
import type { SettingsWorkspaceRoute } from '../settings/SettingsWorkspaceRoute.js';
import {
  WhatsNewPresenter,
  type SeenReleaseStore,
} from '../core/release_notes/WhatsNewPresenter.js';
import { WhatsNewModal } from '../dialogs/WhatsNewModal.js';
import { InputModal } from '../dialogs/InputModal.js';
import { ConfirmDialog } from '../dialogs/ConfirmDialog.js';
import {
  registerWildcardSettings,
  type WildcardSettingsOptions,
} from '../wildcards/presentation/WildcardSettings.js';
import { WhatsNewSeenSetting } from '../settings/WhatsNewSeenSetting.js';

export interface WorkflowFieldHostOptions
  extends Omit<RelationHostOptions, 'reportError' | 'appearance'> {
  reportRelationError(error: unknown): void;
  reportPresentationError(error: unknown): void;
  /** Shows what the host has to tell the person; each answers whether it reached them. */
  hostNotices: {
    pushLastingToast(severity: 'warn', title: string, message: string): boolean;
    pushToast(severity: 'success' | 'info' | 'warn', title: string, message: string): void;
    showHostLine(line: HostLine): boolean;
  };
  activity: WorkflowOutputActivity;
  progress: WorkflowProgressStore;
  stackControls(): CubeStackFrameControls;
  connected(): boolean;
  subscribeConnection(listener: (connected: boolean) => void): () => void;
  /** The page's cache registry, joined to a desktop host's memory budget when one exists. */
  memory: MemoryBudgetRegistry;
  /** The wildcard folder's file actions, offered in SugarCubes' settings. */
  wildcards: Omit<WildcardSettingsOptions, 'app' | 'document' | 'prompt'>;
}

/** Share seed reuse across actual native field mounts and the override toolbar. */
export function createWorkflowFieldHost(options: WorkflowFieldHostOptions): {
  appearance: AppearanceHost;
  relations: RelationHostLifecycle;
  frame: SubstituteFrameHost;
  canvas: EmbeddedCanvasConnection;
  host: HostDescriptionState;
  /** The page's one update source; SugarCubes' own updater attaches here without a host one. */
  updates: UpdateSourceSlot;
  /** The page's one What's new view, for every product whose notes this page shows. */
  whatsNew: WhatsNewPresenter;
  settingsRoute: SettingsWorkspaceRoute;
  setupPresentation(): void;
} {
  const embedded = new EmbeddedPresentation();
  const hostAccent = new HostAccent();
  const host = new HostDescriptionState();
  const updates = new UpdateSourceSlot();
  const draggedFiles = new DraggedFiles();
  const confirm = (title: string, message: string, confirmLabel: string): Promise<boolean> =>
    // A dialog is told which page it belongs to; without one it has nowhere to appear.
    new ConfirmDialog({
      adapter: {
        getDocument: () => options.document,
        getWindow: () => options.document.defaultView,
      },
    }).open({ title, message, confirmLabel });
  const backend = new BackendOutageNotice({
    confirm,
    toast: (severity, summary, detail) => options.hostNotices.pushToast(severity, summary, detail),
    lastingToast: (severity, summary, detail) =>
      void options.hostNotices.pushLastingToast(severity, summary, detail),
    report: options.reportPresentationError,
  });
  const updateReport = new HostUpdateReport();
  const whatsNew = new WhatsNewPresenter(
    new WhatsNewModal({ document: options.document }),
    seenReleases(options.app, options.reportPresentationError),
    options.reportPresentationError,
  );
  const canvas = new EmbeddedCanvasConnection();
  const wildcardNames = new InputModal({
    adapter: {
      getDocument: () => options.document,
      getWindow: () => options.document.defaultView,
    },
  });
  const relations: RelationHostLifecycle = new RelationHostLifecycle({
    ...options,
    reportError: options.reportRelationError,
    appearance: () => ownedAppearance,
  });
  const appearance: AppearanceHost = new AppearanceHost({
    app: options.app,
    document: options.document,
    seedReuse: (node, name) => relations.seedReuse.field(node, name),
    exactSeeds: (node, name) => relations.exactSeeds.field(node, name),
    reportError: options.reportPresentationError,
    embedded,
    hostAccent,
    host,
  });
  const ownedAppearance = new OwnedSurfaceAppearance(options.document, appearance.state);
  const frame = new SubstituteFrameHost({
    app: options.app,
    document: options.document,
    palette: appearance.palette,
    hostColorScheme: appearance.hostColorScheme,
    sessions: options.sessions,
    activity: options.activity,
    progress: options.progress,
    draggedFiles,
    stackControls: options.stackControls,
    openOverrides: (trigger) => relations.openOverrideMenu(trigger),
    setOverridesInline: (inline) => relations.setOverridesInline(inline),
    adoptJobSeeds: (seeds) => relations.jobSeeds.adopt(seeds),
    confirm,
    backend,
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
    updates,
    whatsNew: () => whatsNew.open(),
    updateReport,
    report: options.reportPresentationError,
  });
  /** Mount field appearance and its frame/navigation presentation after native workspace setup. */
  function setupPresentation(): void {
    appearance.setup();
    frame.setup();
    relations.setupOverridePresentation(embedded);
    editorNavigation.setup();
    settingsWorkspace.setup();
    registerWildcardSettings({
      ...options.wildcards,
      app: options.app,
      document: options.document,
      prompt: (prompt) => wildcardNames.open(prompt),
    });
    const windowRef = options.document.defaultView;
    if (windowRef)
      startEmbeddedPanel({
        window: windowRef,
        host,
        updates,
        releaseNotes: whatsNew,
        draggedFiles,
        backend,
        updateReport,
        sessionNotice: {
          notSaved: (lost) => {
            const words = unsavedWorkWords(lost);
            return options.hostNotices.pushLastingToast('warn', words.title, words.message);
          },
          line: (line) => options.hostNotices.showHostLine(line),
        },
        presentation: embedded,
        hostAccent,
        canvas,
        memory: {
          registry: options.memory,
          preferences: () => new MemoryBudgetSettings(options.app),
        },
        connected: options.connected,
        subscribeConnection: options.subscribeConnection,
        report: options.reportPresentationError,
      });
    else host.settle(DeclaredHost.standalone);
  }
  return {
    appearance,
    relations,
    frame,
    canvas,
    host,
    updates,
    whatsNew,
    settingsRoute: settingsWorkspace.sessionRoute,
    setupPresentation,
  };
}

/**
 * Keep seen versions in Comfy's settings; without them nothing counts as an update, so the
 * view opens only when the user asks for it.
 */
function seenReleases(app: unknown, report: (error: unknown) => void): SeenReleaseStore {
  try {
    return new WhatsNewSeenSetting(app);
  } catch (error: unknown) {
    report(error);
    return { read: () => null, record: () => undefined };
  }
}
