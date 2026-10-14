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
/** Own accent persistence, external CSS overrides, and palette-only invalidation. */
import { isRecord } from '../../types/common.js';
import { readComfyDarkTheme } from './ComfyColorScheme.js';
import { DEFAULT_ACCENT_COLOR, normalizeAccentColor } from './AccentPalette.js';
import { createAccentColorSetting } from './AccentColorSetting.js';
import { accentThemeCss } from './AccentThemeStyles.js';
/** Persist the user's accent independently of every workflow document. */
export const ACCENT_COLOR_SETTING = 'SugarCubes.Appearance.AccentColor';
/** Project one accepted accent into mounted and future surfaces without rebuilding graph inventories. */
export class AccentThemeHost {
    #listeners = new Set();
    #theme = Object.freeze({
        color: DEFAULT_ACCENT_COLOR,
        dark: true,
    });
    #document;
    #app;
    #report;
    #style;
    #observer;
    #configured = DEFAULT_ACCENT_COLOR;
    #signature = '';
    #queued = false;
    #disposed = false;
    #setup = false;
    /** Expose the authoritative accepted palette independently of node renderer presentation. */
    get theme() {
        return this.#theme;
    }
    /** Observe palette changes without introducing another document observer or setting owner. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Bind the host-facing settings and CSS boundary without claiming external variables. */
    constructor(documentRef, app, reportError) {
        this.#document = documentRef;
        this.#app = app;
        this.#report = reportError;
        this.#style = documentRef.createElement('style');
        this.#style.id = 'sugarcubes-accent-theme';
        this.#observer = new MutationObserver((records) => {
            if (records.every((record) => record.target === this.#style || this.#style.contains(record.target)))
                return;
            this.#schedule();
        });
    }
    /** Register the color picker and observe only host palette inputs and stylesheet lifecycles. */
    setup() {
        if (this.#setup || this.#disposed)
            return;
        this.#setup = true;
        const settings = this.#settings();
        if (typeof settings.getSettingValue === 'function')
            this.#configured =
                normalizeAccentColor(settings.getSettingValue.call(settings, ACCENT_COLOR_SETTING)) ??
                    DEFAULT_ACCENT_COLOR;
        if (typeof settings.addSetting === 'function')
            settings.addSetting.call(settings, {
                id: ACCENT_COLOR_SETTING,
                name: 'Accent color',
                category: ['SugarCubes', 'Appearance', 'AccentColor'],
                defaultValue: DEFAULT_ACCENT_COLOR,
                tooltip: 'Choose the shared SugarCubes accent. Hosts can override it with --sugarcubes-accent on the document root.',
                type: () => createAccentColorSetting(this.#document, this.#configured, (color) => {
                    if (typeof settings.setSettingValue !== 'function') {
                        this.#report(new Error('Comfy accent setting persistence is unavailable.'));
                        return;
                    }
                    try {
                        void Promise.resolve(settings.setSettingValue.call(settings, ACCENT_COLOR_SETTING, color)).catch(this.#report);
                    }
                    catch (error) {
                        this.#report(error);
                    }
                }),
                onChange: (value) => {
                    const color = normalizeAccentColor(value);
                    if (!color) {
                        this.#report(new TypeError('SugarCubes accent must be a hexadecimal color such as #b58cff.'));
                        return;
                    }
                    this.#configured = color;
                    this.refresh();
                },
            });
        this.#document.head.append(this.#style);
        this.#document.head.addEventListener('load', this.#onStylesheetLoad, true);
        for (const root of [this.#document.documentElement, this.#document.body])
            this.#observer.observe(root, { attributes: true, attributeFilter: ['class', 'style'] });
        this.#observer.observe(this.#document.head, {
            childList: true,
            subtree: true,
            characterData: true,
            attributes: true,
            attributeFilter: ['href', 'media', 'disabled'],
        });
        this.refresh();
    }
    /** Resolve external CSS before the saved preference and publish only an actual palette change. */
    refresh() {
        if (this.#disposed || !this.#setup)
            return;
        const style = this.#document.defaultView?.getComputedStyle(this.#document.documentElement);
        const color = normalizeAccentColor(style?.getPropertyValue('--sugarcubes-accent')) ?? this.#configured;
        const dark = readComfyDarkTheme(this.#document);
        const signature = `${color}:${dark}`;
        if (signature === this.#signature)
            return;
        this.#signature = signature;
        this.#style.textContent = accentThemeCss(color, dark);
        this.#theme = Object.freeze({ color, dark });
        for (const listener of this.#listeners)
            listener();
        const app = isRecord(this.#app) ? this.#app : {};
        const canvas = isRecord(app.canvas) ? app.canvas : {};
        if (typeof canvas.setDirty === 'function')
            canvas.setDirty.call(canvas, true, true);
    }
    /** Release presentation overrides while preserving saved preferences and external host styles. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#listeners.clear();
        this.#observer.disconnect();
        this.#document.head.removeEventListener('load', this.#onStylesheetLoad, true);
        this.#style.remove();
    }
    /** Coalesce styles and palette events without observing native controls or graph topology. */
    #schedule() {
        if (this.#queued || this.#disposed)
            return;
        this.#queued = true;
        queueMicrotask(() => {
            this.#queued = false;
            try {
                this.refresh();
            }
            catch (error) {
                this.#report(error);
            }
        });
    }
    /** Resolve a newly loaded external stylesheet after its rules join the cascade. */
    #onStylesheetLoad = (event) => {
        if (event.target instanceof HTMLLinkElement && event.target.rel === 'stylesheet')
            this.#schedule();
    };
    /** Narrow dynamic Comfy settings before accepting host operations. */
    #settings() {
        const app = isRecord(this.#app) ? this.#app : {};
        const ui = isRecord(app.ui) ? app.ui : {};
        return isRecord(ui.settings) ? ui.settings : {};
    }
}
