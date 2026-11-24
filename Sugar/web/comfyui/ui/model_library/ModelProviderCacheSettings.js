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
 * The Model Sources page's Provider cache rows, as in Python: CivitAI cache usage, then its
 * maintenance actions. The server keeps the cache and reports its usage.
 */
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
import { SettingsState } from '../settings/workspace/SettingsModel.js';
/** Keep cache activity independent of preference edits. */
export class ModelProviderCacheSettings {
    client;
    report;
    #summary = new SettingsState('Loading cache usage…');
    #idle = new SettingsState(false);
    #registered = false;
    constructor(client, report) {
        this.client = client;
        this.report = report;
    }
    /** Contribute the usage and maintenance rows and read the usage. */
    setup(app) {
        if (this.#registered)
            return;
        this.#registered = true;
        const catalog = settingsCatalogFor(app);
        catalog.contribute('model_sources.provider_cache.civitai_usage', {
            controls: [{ kind: 'status', text: this.#summary }],
        });
        catalog.contribute('model_sources.provider_cache.civitai_maintenance', {
            controls: [
                ['clear_thumbnails', 'Clear thumbnails'],
                ['clear_metadata', 'Clear metadata'],
                ['refresh', 'Refresh'],
            ].map(([action, label]) => ({
                kind: 'button',
                button: { label, run: () => this.#execute(action), enabled: this.#idle },
            })),
        });
        void this.client
            .load()
            .then((value) => this.#show(value))
            .catch((error) => {
            this.#summary.set('Cache usage unavailable.');
            this.#idle.set(true);
            this.report(error);
        });
    }
    /** Run one action, showing its progress, and keep the others off until it returns. */
    async #execute(action) {
        this.#idle.set(false);
        try {
            this.#show(await this.client.execute(action, (message) => this.#summary.set(message)));
        }
        catch (error) {
            this.#summary.set('Cache action did not complete.');
            this.#idle.set(true);
            throw error;
        }
    }
    /** The server's exact record and image counts. */
    #show(value) {
        const bytes = value.thumbnailBytes;
        const size = bytes < 1024
            ? `${bytes} B`
            : bytes < 1024 ** 2
                ? `${(bytes / 1024).toFixed(1)} KB`
                : `${(bytes / 1024 ** 2).toFixed(1)} MB`;
        this.#summary.set(`${value.providerRecordCount} metadata records · ${value.thumbnailSourceCount} thumbnails · ${value.thumbnailVariantCount} variants · ${size}`);
        this.#idle.set(true);
    }
}
