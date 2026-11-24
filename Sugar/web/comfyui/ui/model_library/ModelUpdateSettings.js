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
 * The Generation page's "Notify me about updates to models I use" row: the server-held consent
 * as a switch, and a check on demand.
 */
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
import { SettingsState } from '../settings/workspace/SettingsModel.js';
/** Show durable opt-in and explicit checks through the shared update application boundary. */
export class ModelUpdateSettings {
    updates;
    report;
    #loaded = new SettingsState(false);
    #registered = false;
    constructor(updates, report) {
        this.updates = updates;
        this.report = report;
    }
    /** Contribute the row and read the server's consent; nothing writes an initial default. */
    setup(app) {
        if (this.#registered)
            return;
        this.#registered = true;
        const subscribe = (listener) => {
            const stopUpdates = this.updates.subscribe(listener);
            const stopLoaded = this.#loaded.subscribe(listener);
            return () => {
                stopUpdates();
                stopLoaded();
            };
        };
        settingsCatalogFor(app).contribute('generation.missing_models.update_notices', {
            controls: [
                {
                    kind: 'button',
                    button: {
                        label: 'Check for updates',
                        run: () => this.updates.check(),
                        enabled: { read: () => this.updates.read().enabled, subscribe },
                    },
                },
                {
                    kind: 'switch',
                    value: {
                        read: () => this.updates.read().enabled,
                        write: async (enabled) => {
                            await this.updates.setEnabled(enabled);
                            if (this.updates.read().enabled)
                                await this.updates.check();
                        },
                        subscribe,
                    },
                },
            ],
            enabled: { read: () => this.#loaded.read(), subscribe },
        });
        void this.updates
            .load()
            .then(() => this.#loaded.set(true))
            .catch(this.report);
    }
}
