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
/** Bind unified settings presentation to named Comfy mounts and existing visual-mode authorities. */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { ComfySettingsCatalog } from './ComfySettingsCatalog.js';
import { SettingsWorkspaceSurface } from './SettingsWorkspaceSurface.js';
import { settingsWorkspaceCss } from './SettingsWorkspaceStyles.js';
import { AboutSettingsPage } from './AboutSettingsPage.js';
import { ComfyAboutInformation } from './ComfyAboutInformation.js';
import { SettingsPopupTheme } from './SettingsPopupTheme.js';
import { AppearanceState } from '../surface/appearance/AppearanceState.js';
/** Preserve Comfy's page lifecycle and settings commands across both browser presentation modes. */
export class SettingsWorkspaceHost {
    options;
    #catalog;
    #surfaces = new Map();
    #stops = [];
    #style;
    #popups = new SettingsPopupTheme();
    #appearance = new AppearanceState();
    #observer = null;
    #about = null;
    #queued = false;
    #started = false;
    #disposed = false;
    /** Prepare presentation independently of workflow contents, graph topology and node renderer. */
    constructor(options) {
        this.options = options;
        this.#catalog = new ComfySettingsCatalog(options.app);
        this.#style = options.document.createElement('style');
        this.#style.id = 'sugarcubes-settings-workspace';
        this.#style.textContent = settingsWorkspaceCss;
    }
    /** Add a native settings category and observe only Comfy's identified settings dialog. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        this.#started = true;
        this.#catalog.register({
            id: 'SugarCubes.About.Information',
            name: 'About SugarCubes',
            category: ['About SugarCubes', 'Information', 'Project'],
            tooltip: 'Project, versions, source code, license, contributors and acknowledgements.',
            defaultValue: '',
            type: () => {
                this.#about ??= new AboutSettingsPage(this.options.document, new ComfyAboutInformation(), {
                    getDocument: () => this.options.document,
                    getWindow: () => this.options.document.defaultView,
                }, this.options.report, this.#appearance);
                this.#about.refresh();
                return this.#about.element;
            },
        });
        this.options.document.head.append(this.#style);
        this.#stops.push(this.options.mode.subscribe(this.#queue), this.options.palette.subscribe(this.#queue));
        this.#observer = new HostSurfaceObserver(this.options.document.body, '[data-testid="settings-dialog"], .p-select-overlay', (changes) => {
            for (const { element } of changes) {
                if (element.matches('.p-select-overlay'))
                    this.#popups.reconcile(element, this.#surfaces.keys(), this.options.mode.enabled, this.options.palette.theme.dark);
                else
                    this.#queue();
            }
        }, { content: true, exclude: '.sc-settings-owned' });
        this.#reconcile();
        this.options.document.defaultView?.addEventListener('pagehide', this.dispose);
    }
    /** Restore mounted native controls and release only this settings presentation lifetime. */
    dispose = () => {
        this.#disposed = true;
        this.#observer?.disconnect();
        for (const stop of this.#stops.splice(0))
            stop();
        for (const surface of this.#surfaces.values())
            surface.dispose();
        this.#surfaces.clear();
        this.#popups.dispose();
        this.#about?.dispose();
        this.#appearance.dispose();
        this.#style.remove();
        this.options.document.defaultView?.removeEventListener('pagehide', this.dispose);
    };
    /** Coalesce scoped native page changes without performing workflow or node discovery. */
    #queue = () => {
        if (this.#queued || this.#disposed)
            return;
        this.#queued = true;
        queueMicrotask(() => {
            this.#queued = false;
            try {
                this.#reconcile();
            }
            catch (error) {
                this.options.report(error);
            }
        });
    };
    /** Annotate only actual settings surfaces, keeping third-party custom renderers untouched. */
    #reconcile() {
        if (this.#disposed)
            return;
        this.#appearance.update(this.options.mode.enabled, this.options.palette.theme.dark);
        for (const [root, surface] of this.#surfaces) {
            if (root.isConnected)
                continue;
            surface.dispose();
            this.#surfaces.delete(root);
        }
        for (const root of this.options.document.querySelectorAll('[data-testid="settings-dialog"]')) {
            let surface = this.#surfaces.get(root);
            if (!surface) {
                surface = new SettingsWorkspaceSurface(root, this.#catalog);
                this.#surfaces.set(root, surface);
            }
            surface.update(this.options.mode.enabled, this.options.palette.theme.dark);
        }
        this.#popups.refresh(this.#surfaces.keys(), this.options.mode.enabled, this.options.palette.theme.dark);
        this.#observer?.discardPending();
    }
}
