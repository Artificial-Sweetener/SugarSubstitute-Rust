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
import { RelationHostLifecycle } from './RelationHostLifecycle.js';
import { AppearanceHost } from '../surface/appearance/AppearanceHost.js';
import { OwnedSurfaceAppearance } from '../surface/appearance/OwnedSurfaceAppearance.js';
import { WorkflowEditorNavigationHost } from '../surface/WorkflowEditorNavigationHost.js';
import { SubstituteFrameHost } from '../surface/frame/SubstituteFrameHost.js';
import { SettingsWorkspaceHost } from '../settings/SettingsWorkspaceHost.js';
import { EmbeddedPresentation } from '../core/embedded/EmbeddedPresentation.js';
import { HostAccent } from '../core/embedded/HostAccent.js';
import { HostDescriptionState } from '../core/embedded/HostDescriptionState.js';
import { DeclaredHost } from '../core/embedded/PanelRuntime.js';
import { startEmbeddedPanel } from '../core/embedded/EmbeddedPanelHost.js';
import { EmbeddedCanvasConnection } from '../core/embedded/EmbeddedCanvasConnection.js';
import { MemoryBudgetSettings } from '../settings/MemoryBudgetSettings.js';
/** Share seed reuse across actual native field mounts and the override toolbar. */
export function createWorkflowFieldHost(options) {
    const embedded = new EmbeddedPresentation();
    const hostAccent = new HostAccent();
    const host = new HostDescriptionState();
    const canvas = new EmbeddedCanvasConnection();
    const relations = new RelationHostLifecycle({
        ...options,
        reportError: options.reportRelationError,
        appearance: () => ownedAppearance,
    });
    const appearance = new AppearanceHost({
        app: options.app,
        document: options.document,
        seedReuse: (node, name) => relations.seedReuse.field(node, name),
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
    function setupPresentation() {
        appearance.setup();
        frame.setup();
        relations.setupOverridePresentation(embedded);
        editorNavigation.setup();
        settingsWorkspace.setup();
        const windowRef = options.document.defaultView;
        if (windowRef)
            startEmbeddedPanel({
                window: windowRef,
                host,
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
        else
            host.settle(DeclaredHost.standalone);
    }
    return {
        appearance,
        relations,
        frame,
        canvas,
        host,
        settingsRoute: settingsWorkspace.sessionRoute,
        setupPresentation,
    };
}
