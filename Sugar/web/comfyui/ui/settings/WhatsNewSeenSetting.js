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
/** Keep the versions the user has been shown What's new for in Comfy's persisted settings. */
import { isRecord } from '../types/common.js';
/** The hidden setting holding `{ [product]: version }`. */
export const WHATS_NEW_SEEN_SETTING_ID = 'SugarCubes.WhatsNew.SeenVersions';
const TOKEN = /^[a-z0-9.-]{1,64}$/;
/** Read and record seen versions through Comfy's settings store. */
export class WhatsNewSeenSetting {
    #settings;
    #values;
    /** Register the hidden setting once; it never appears in the settings dialog. */
    constructor(app) {
        const host = isRecord(app) ? app : {};
        const ui = isRecord(host.ui) ? host.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : null;
        if (!settings ||
            typeof settings.addSetting !== 'function' ||
            typeof settings.setSettingValue !== 'function')
            throw new Error("Comfy settings are unavailable for What's new.");
        this.#settings = settings;
        settings.addSetting.call(settings, {
            id: WHATS_NEW_SEEN_SETTING_ID,
            name: "What's new: versions already shown",
            type: 'hidden',
            defaultValue: {},
        });
        this.#values = this.#stored();
    }
    read(product) {
        return this.#values[product] ?? null;
    }
    record(product, version) {
        if (!TOKEN.test(version))
            return;
        this.#values = { ...this.#stored(), [product]: version };
        this.#settings.setSettingValue.call(this.#settings, WHATS_NEW_SEEN_SETTING_ID, this.#values);
    }
    /** The stored map with anything that is not a product's version dropped. */
    #stored() {
        const get = this.#settings.getSettingValue;
        const value = typeof get === 'function'
            ? get.call(this.#settings, WHATS_NEW_SEEN_SETTING_ID)
            : undefined;
        const values = {};
        if (!isRecord(value))
            return values;
        for (const [product, version] of Object.entries(value))
            if (typeof version === 'string' && TOKEN.test(version))
                values[product] = version;
        return values;
    }
}
