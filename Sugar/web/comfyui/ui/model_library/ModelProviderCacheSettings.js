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
/** Present cache usage and reference maintenance actions through the shared server owner. */
import { sugarCubesSettingsSection } from '../settings/SugarCubesSettingsSections.js';
import { isRecord } from '../types/common.js';
/** Keep cache activity independent of preference edits and their form state. */
export class ModelProviderCacheSettings {
    #document;
    #client;
    #report;
    #row = null;
    #summary = null;
    #buttons = [];
    #loading = null;
    #busy = false;
    #registered = false;
    /** Bind transport and existing non-blocking feedback without storing cache data in settings. */
    constructor(document, client, report) {
        this.#document = document;
        this.#client = client;
        this.#report = report;
    }
    /** Register one cache row with a usage summary and the three deliberate maintenance actions. */
    setup(app) {
        if (this.#registered)
            return;
        const host = isRecord(app) ? app : {};
        const ui = isRecord(host.ui) ? host.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : {};
        if (typeof settings.addSetting !== 'function')
            throw new Error('Comfy model cache settings are unavailable.');
        settings.addSetting.call(settings, {
            id: 'SugarCubes.Models.CivitaiCache',
            name: 'CivitAI cache',
            ...sugarCubesSettingsSection('metadata', 'CivitaiCache'),
            defaultValue: '',
            type: () => {
                if (!this.#row)
                    this.#create();
                this.#load();
                return this.#row;
            },
        });
        this.#registered = true;
    }
    /** Render literal usage and bounded controls within the existing settings layout. */
    #create() {
        this.#row = this.#document.createElement('div');
        this.#row.className = 'sugarcubes-settings-row';
        this.#row.style.cssText =
            'display:flex;flex-direction:column;align-items:flex-start;gap:8px;min-width:0';
        this.#summary = this.#document.createElement('span');
        this.#summary.setAttribute('aria-live', 'polite');
        this.#summary.textContent = 'Loading cache usage…';
        const actions = this.#document.createElement('div');
        actions.style.cssText = 'display:flex;flex-wrap:wrap;gap:8px;max-width:100%';
        for (const [action, caption] of [
            ['clear_thumbnails', 'Clear thumbnails'],
            ['clear_metadata', 'Clear metadata'],
            ['refresh', 'Refresh'],
        ]) {
            const button = this.#document.createElement('button');
            button.type = 'button';
            button.className = 'p-button p-component p-button-secondary';
            button.textContent = caption;
            button.disabled = true;
            button.addEventListener('click', () => this.#execute(action));
            this.#buttons.push(button);
            actions.append(button);
        }
        this.#row.append(this.#summary, actions);
    }
    /** Re-read usage on settings entry and allow retry after unavailable transport. */
    #load() {
        if (this.#loading || this.#busy)
            return;
        this.#loading = this.#client
            .load()
            .then((value) => this.#show(value))
            .catch((error) => {
            if (this.#summary)
                this.#summary.textContent = 'Cache usage unavailable.';
            this.#report(error);
        })
            .finally(() => {
            this.#loading = null;
        });
    }
    /** Disable competing gestures until the shared job returns committed usage. */
    #execute(action) {
        if (this.#busy || this.#loading)
            return;
        this.#busy = true;
        for (const button of this.#buttons)
            button.disabled = true;
        void this.#client
            .execute(action, (message) => {
            if (this.#summary)
                this.#summary.textContent = message;
        })
            .then((value) => this.#show(value))
            .catch((error) => {
            if (this.#summary)
                this.#summary.textContent = 'Cache action did not complete.';
            this.#report(error);
        })
            .finally(() => {
            this.#busy = false;
            for (const button of this.#buttons)
                button.disabled = false;
        });
    }
    /** Show the server's exact record and image counts without manufacturing empty defaults. */
    #show(value) {
        const bytes = value.thumbnailBytes;
        const size = bytes < 1024
            ? `${bytes} B`
            : bytes < 1024 ** 2
                ? `${(bytes / 1024).toFixed(1)} KB`
                : `${(bytes / 1024 ** 2).toFixed(1)} MB`;
        if (this.#summary)
            this.#summary.textContent = `${value.providerRecordCount} metadata records · ${value.thumbnailSourceCount} thumbnails · ${value.thumbnailVariantCount} variants · ${size}`;
        for (const button of this.#buttons)
            button.disabled = this.#busy;
    }
}
