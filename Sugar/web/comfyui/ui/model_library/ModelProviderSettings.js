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
 * The CivitAI provider policy's settings rows: missing-model lookup and downloads on the
 * Generation page; local metadata lookup, thumbnail downloads and thumbnail safety on the Model
 * Sources page. The server keeps the policy; the rows show it and send each change.
 */
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
import { modelThumbnailSafetyChoices, } from './ModelProviderPreferences.js';
/** Show provider policy without manufacturing defaults or persisting browser-local choices. */
export class ModelProviderSettings {
    client;
    report;
    #listeners = new Set();
    #value = null;
    #busy = false;
    #registered = false;
    constructor(client, report) {
        this.client = client;
        this.report = report;
    }
    /** Contribute the rows and read the current policy. */
    setup(app) {
        if (this.#registered)
            return;
        this.#registered = true;
        const catalog = settingsCatalogFor(app);
        const toggles = [
            ['generation.missing_models.lookup', 'missingModelLookupEnabled'],
            ['generation.missing_models.downloads', 'downloadsEnabled'],
            ['model_sources.civitai_lookup.local_metadata', 'metadataLookupEnabled'],
            ['model_sources.safety.civitai_thumbnails', 'thumbnailDownloadsEnabled'],
        ];
        for (const [row, key] of toggles)
            catalog.contribute(row, {
                controls: [{ kind: 'switch', value: this.#field(key, (value) => value[key]) }],
                enabled: this.#ready(() => true),
            });
        catalog.contribute('model_sources.safety.civitai_thumbnail_policy', {
            controls: [
                {
                    kind: 'choice',
                    value: this.#field('thumbnailSafety', (value) => value.thumbnailSafety),
                    choices: {
                        read: () => modelThumbnailSafetyChoices.map(({ value, label }) => ({ value, label })),
                        subscribe: () => () => undefined,
                    },
                },
            ],
            // Thumbnail safety applies only while thumbnails are downloaded.
            enabled: this.#ready((value) => value.thumbnailDownloadsEnabled),
        });
        catalog.contribute('model_sources.safety.civitai_thumbnail_reveal', {
            controls: [
                {
                    kind: 'switch',
                    value: this.#field('thumbnailRevealOnHover', (value) => value.thumbnailRevealOnHover),
                },
            ],
            // There is something to reveal only while thumbnails are shown blurred.
            enabled: this.#ready((value) => value.thumbnailDownloadsEnabled && value.thumbnailSafety === 'blur'),
        });
        this.#load();
    }
    /** A live value for one policy field, written as one change to the server. */
    #field(key, read) {
        return {
            read: () => (this.#value ? read(this.#value) : false),
            write: (next) => this.#configure({ [key]: next }),
            subscribe: (listener) => this.#subscribe(listener),
        };
    }
    /** Controls work once the policy is known and no change is in flight. */
    #ready(when) {
        return {
            read: () => !this.#busy && this.#value !== null && when(this.#value),
            subscribe: (listener) => this.#subscribe(listener),
        };
    }
    #configure(patch) {
        if (this.#busy || !this.#value)
            return Promise.resolve();
        this.#busy = true;
        this.#changed();
        return this.client
            .configure(patch)
            .then((value) => {
            this.#value = value;
        })
            .finally(() => {
            this.#busy = false;
            this.#changed();
        });
    }
    #load() {
        void this.client
            .load()
            .then((value) => {
            this.#value = value;
            this.#changed();
        })
            .catch(this.report);
    }
    #subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    #changed() {
        for (const listener of [...this.#listeners])
            listener();
    }
}
