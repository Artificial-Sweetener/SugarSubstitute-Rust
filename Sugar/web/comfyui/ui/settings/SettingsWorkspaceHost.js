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
/**
 * Present the settings workspace in ComfyUI's settings dialog. In the SugarSubstitute frame the
 * workspace is the whole dialog; in plain ComfyUI it is SugarCubes' one settings row, drawn
 * through ComfyUI's custom setting renderer, and nothing else of ComfyUI's dialog is touched.
 */
import { subscribeVisualStyle, readOwnedVisualStyle, } from '../surface/appearance/VisualStyleBoundary.js';
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { AboutSettingsPage } from './AboutSettingsPage.js';
import { UpdatesSettingsPage } from './UpdatesSettingsPage.js';
import { ComfyAboutInformation } from './ComfyAboutInformation.js';
import { AppearanceState } from '../surface/appearance/AppearanceState.js';
import { isRecord } from '../types/common.js';
import { settingsCatalogFor } from './workspace/SettingsCatalog.js';
import { ComfySettingStore } from './workspace/ComfySettingStore.js';
import { ComfySettingsDialog, HOST_ATTRIBUTE, PANEL_ATTRIBUTE, } from './workspace/ComfySettingsDialog.js';
import { SettingsWorkspaceView } from './workspace/SettingsWorkspaceView.js';
import { settingsWorkspaceCss } from './workspace/SettingsWorkspaceStyles.js';
import { fixed } from './workspace/SettingsModel.js';
/** The one ComfyUI setting that carries the workspace into plain ComfyUI's dialog. */
export const SETTINGS_ROW_ID = 'SugarCubes.Settings';
/** ComfyUI's store of the keys a person has bound. */
const KEYBINDINGS_SETTING = 'Comfy.Keybinding.NewBindings';
/** ComfyUI's live preview method, sent with every prompt. */
const PREVIEW_SETTING = 'Comfy.Execution.PreviewMethod';
/** The Controls page's keyboard rows and the SugarCubes command each one binds. */
const KEYBOARD_ROWS = [
    ['controls.keyboard.cube_stack', 'SugarCubes.CubeStackCart'],
    ['controls.keyboard.masonry', 'SugarCubes.ToggleMasonry'],
    ['controls.keyboard.output_canvas', 'SugarCubes.ToggleOutputCanvas'],
    ['controls.keyboard.console', 'SugarCubes.ToggleConsole'],
];
/** A command's bound key as ComfyUI stores it, written as Python wrote shortcuts. */
export function describeBinding(bindings, commandId) {
    if (!Array.isArray(bindings))
        return 'Not assigned';
    const binding = [...bindings]
        .reverse()
        .find((candidate) => isRecord(candidate) && candidate.commandId === commandId);
    const combo = isRecord(binding) && isRecord(binding.combo) ? binding.combo : null;
    if (!combo || typeof combo.key !== 'string')
        return 'Not assigned';
    const key = combo.key.length === 1 ? combo.key.toUpperCase() : combo.key;
    return [
        combo.ctrl && 'Ctrl',
        combo.alt && 'Alt',
        combo.shift && 'Shift',
        combo.meta && 'Meta',
        key,
    ]
        .filter((part) => typeof part === 'string' && part !== '')
        .join('+');
}
/** Preserve ComfyUI's dialog lifecycle and settings command across both presentations. */
export class SettingsWorkspaceHost {
    options;
    /** Expose the actual mounted route to desktop checkpoint coordination. */
    sessionRoute = {
        read: () => this.#route,
        subscribe: (listener) => {
            this.#routeListeners.add(listener);
            return () => {
                this.#routeListeners.delete(listener);
            };
        },
        show: (signal) => this.#show(signal),
    };
    #routeListeners = new Set();
    #route = 'workflow';
    #shows = new Set();
    #store;
    #frames = new Map();
    #page = null;
    #stops = [];
    #style;
    #appearance = new AppearanceState();
    #observer = null;
    #about = null;
    #updatesPage = null;
    #queued = false;
    #started = false;
    #disposed = false;
    constructor(options) {
        this.options = options;
        this.#store = new ComfySettingStore(options.app);
        this.#style = options.document.createElement('style');
        this.#style.id = 'sugarcubes-settings-workspace';
        this.#style.textContent = settingsWorkspaceCss;
    }
    /** Register the workspace row, contribute About and Updates, and follow settings dialogs. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        this.#started = true;
        const catalog = settingsCatalogFor(this.options.app);
        this.#store.register({
            id: SETTINGS_ROW_ID,
            name: 'SugarCubes settings',
            category: ['SugarCubes', 'SugarCubes', 'Settings'],
            defaultValue: '',
            type: () => {
                this.#page ??= this.#view('page', null);
                return this.#page.element;
            },
        });
        this.#stops.push(catalog.contribute('about.page', {
            body: () => {
                this.#about ??= new AboutSettingsPage(this.options.document, new ComfyAboutInformation(), {
                    getDocument: () => this.options.document,
                    getWindow: () => this.options.document.defaultView,
                }, this.options.report, this.#appearance, this.#aboutHost());
                this.#about.refresh();
                return this.#about.element;
            },
        }));
        if (this.options.updates) {
            const updates = this.options.updates;
            const available = {
                read: () => updates.read() !== null,
                subscribe: (listener) => updates.subscribe(listener),
            };
            this.#stops.push(catalog.contribute('about.updates', {
                visible: available,
                body: () => {
                    this.#updatesPage ??= new UpdatesSettingsPage(this.options.document, updates, this.options.report, undefined, this.options.whatsNew ?? null, this.options.updateReport ?? null);
                    return this.#updatesPage.element;
                },
            }));
        }
        this.#stops.push(catalog.contribute('generation.preview.configuration', this.#previews()));
        for (const [row, commandId] of KEYBOARD_ROWS)
            this.#stops.push(catalog.contribute(row, {
                controls: [
                    {
                        kind: 'status',
                        text: {
                            read: () => describeBinding(this.#store.get(KEYBINDINGS_SETTING), commandId),
                            subscribe: (listener) => this.#store.subscribe(KEYBINDINGS_SETTING, listener),
                        },
                    },
                    { kind: 'button', button: { label: 'Change…', run: () => this.#openKeybindings() } },
                ],
            }));
        this.options.document.head.append(this.#style);
        this.#stops.push(this.options.mode.subscribe(this.#queue), this.options.palette.subscribe(this.#queue), subscribeVisualStyle(this.options.document, this.#queue));
        this.#observer = new HostSurfaceObserver(this.options.document.body, '[data-testid="settings-dialog"]', () => this.#queue(), { exclude: '.sc-sw' });
        this.#reconcile();
        this.options.document.defaultView?.addEventListener('pagehide', this.dispose);
    }
    /** Remove every presentation and restore ComfyUI's dialog. */
    dispose = () => {
        this.#disposed = true;
        for (const reject of [...this.#shows])
            reject(new Error('Settings workspace retired.'));
        this.#observer?.disconnect();
        for (const stop of this.#stops.splice(0))
            stop();
        for (const [root, view] of this.#frames)
            this.#retire(root, view);
        this.#frames.clear();
        this.#page?.dispose();
        this.#page = null;
        this.#about?.dispose();
        this.#updatesPage?.dispose();
        this.#appearance.dispose();
        this.#style.remove();
        this.#routeListeners.clear();
        this.options.document.defaultView?.removeEventListener('pagehide', this.dispose);
    };
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
    /**
     * Python's Generation previews switch expander over ComfyUI's live preview method: on for any
     * method but `none`, with the method as its Preview type.
     */
    /** The desktop's identity for About: whether it hosts the page, and its running version. */
    #aboutHost() {
        const { mode, updates } = this.options;
        const hostSource = () => {
            const source = updates?.read() ?? null;
            return source?.applies === 'host-restart' ? source : null;
        };
        return {
            read: () => ({
                desktop: mode.desktop,
                version: hostSource()?.read()?.currentVersion ?? null,
            }),
            subscribe: (listener) => {
                let stopSource = hostSource()?.subscribe(listener) ?? null;
                const stopMode = mode.subscribe(listener);
                const stopSlot = updates?.subscribe(() => {
                    stopSource?.();
                    stopSource = hostSource()?.subscribe(listener) ?? null;
                    listener();
                }) ?? null;
                return () => {
                    stopMode();
                    stopSlot?.();
                    stopSource?.();
                };
            },
        };
    }
    #previews() {
        const method = this.#store.value(PREVIEW_SETTING, (value) => typeof value === 'string' ? value : 'default');
        let lastOn = 'auto';
        const on = {
            read: () => method.read() !== 'none',
            write: (enabled) => {
                if (!enabled && method.read() !== 'none')
                    lastOn = method.read();
                return method.write(enabled ? lastOn : 'none');
            },
            subscribe: (listener) => method.subscribe(listener),
        };
        return {
            expander: {
                switch: on,
                rows: [
                    {
                        id: 'generation.preview.type',
                        title: 'Preview type',
                        description: 'Latent preview method sent with new prompts.',
                        controls: [
                            {
                                kind: 'choice',
                                value: method,
                                choices: fixed([
                                    { value: 'default', label: 'Server default' },
                                    { value: 'auto', label: 'Auto' },
                                    { value: 'latent2rgb', label: 'Latent RGB' },
                                    { value: 'taesd', label: 'TAESD' },
                                ]),
                            },
                        ],
                    },
                ],
            },
            visible: {
                read: () => this.#store.definitions().some((definition) => definition.id === PREVIEW_SETTING),
                subscribe: () => () => undefined,
            },
        };
    }
    /**
     * Keys are captured in ComfyUI's Keybinding panel, which no extension can draw: the frame's
     * workspace opens it in place; on a plain page, ComfyUI's own entry opens it.
     */
    #openKeybindings() {
        if (this.#frames.size) {
            for (const view of this.#frames.values())
                view.open('keybinding');
            return;
        }
        const entry = [
            ...this.options.document.querySelectorAll('[data-testid="settings-dialog"] [data-nav-id]'),
        ].find((item) => item.dataset.navId === 'keybinding');
        entry?.click();
    }
    #view(host, root) {
        const view = new SettingsWorkspaceView(this.options.document, host, settingsCatalogFor(this.options.app), this.options.report, root ? new ComfySettingsDialog(root, this.#store) : null);
        view.setAppearance(readOwnedVisualStyle(this.options.document), this.options.palette.theme.dark);
        return view;
    }
    #retire(root, view) {
        view.dispose();
        root.removeAttribute(HOST_ATTRIBUTE);
        root.removeAttribute(PANEL_ATTRIBUTE);
    }
    /** Present every mounted dialog for the current mode, style and palette. */
    #reconcile() {
        if (this.#disposed)
            return;
        const style = readOwnedVisualStyle(this.options.document);
        const dark = this.options.palette.theme.dark;
        this.#appearance.update(style, dark);
        this.#page?.setAppearance(style, dark);
        const roots = [
            ...this.options.document.querySelectorAll('[data-testid="settings-dialog"]'),
        ];
        for (const [root, view] of this.#frames)
            if (!root.isConnected || !this.options.mode.enabled) {
                this.#retire(root, view);
                this.#frames.delete(root);
            }
        if (this.options.mode.enabled)
            for (const root of roots) {
                let view = this.#frames.get(root);
                if (!view) {
                    view = this.#view('frame', root);
                    this.#frames.set(root, view);
                    root.setAttribute(HOST_ATTRIBUTE, '');
                    root.append(view.element);
                    view.focusSearch();
                }
                else
                    view.setAppearance(style, dark);
            }
        const route = roots.length ? 'settings' : 'workflow';
        if (this.#route !== route) {
            this.#route = route;
            for (const listener of this.#routeListeners)
                listener();
        }
        this.#observer?.discardPending();
    }
    /** Observe the dialog mounting before acknowledging restoration; cancellation owns every wait. */
    async #show(signal) {
        if (this.#disposed || signal?.aborted)
            throw new Error('Settings workspace request was cancelled.');
        if (this.#route === 'settings')
            return;
        const app = this.options.app;
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
        const command = manager && isRecord(manager.command) ? manager.command : null;
        const window = this.options.document.defaultView;
        if (!window || !command || typeof command.execute !== 'function')
            throw new Error('Comfy settings command is unavailable.');
        const execute = command.execute;
        await new Promise((resolve, reject) => {
            let done = false;
            const finish = (error) => {
                if (done)
                    return;
                done = true;
                this.#shows.delete(fail);
                this.#routeListeners.delete(changed);
                window.clearTimeout(timer);
                signal?.removeEventListener('abort', cancelled);
                if (error)
                    reject(error);
                else
                    resolve();
            };
            const fail = (error) => finish(error);
            const changed = () => {
                if (this.#route === 'settings')
                    finish();
            };
            const cancelled = () => finish(new Error('Settings workspace request was cancelled.'));
            const timer = window.setTimeout(() => finish(new Error('Comfy settings did not mount before the restore deadline.')), 15000);
            this.#shows.add(fail);
            this.#routeListeners.add(changed);
            signal?.addEventListener('abort', cancelled, { once: true });
            Promise.resolve()
                .then(() => {
                if (!done)
                    return execute.call(command, 'Comfy.ShowSettingsDialog');
                return undefined;
            })
                .catch((error) => finish(error instanceof Error
                ? error
                : new Error('Comfy settings command failed.', { cause: error })));
        });
    }
}
