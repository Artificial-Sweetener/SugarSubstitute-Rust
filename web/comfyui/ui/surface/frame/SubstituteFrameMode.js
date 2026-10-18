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
/** Own the independent frame-mode preference and its explicit host selection boundary. */
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import { isRecord } from '../../types/common.js';
/** Persist only the opt-in visual-mode preference in Comfy settings. */
export const SUBSTITUTE_FRAME_SETTING = 'SugarCubes.Appearance.SubstituteFrame';
/** Publish presentation choices without modifying workflow or node-renderer state. */
export class SubstituteFrameMode {
    #listeners = new Set();
    #enabled = false;
    #started = false;
    #desktop = false;
    /** Read the accepted mode used by frame and rail presentation. */
    get enabled() {
        return this.#enabled || this.#desktop;
    }
    /** Accept an explicit choice from settings or a future negotiated host integration. */
    setEnabled(enabled) {
        const previous = this.enabled;
        this.#enabled = enabled;
        if (this.enabled === previous)
            return;
        for (const listener of this.#listeners)
            listener();
    }
    /** Keep desktop chrome mounted while retaining the independent persisted browser preference. */
    setDesktop(enabled) {
        const previous = this.enabled;
        this.#desktop = enabled;
        if (this.enabled === previous)
            return;
        for (const listener of this.#listeners)
            listener();
    }
    /** Observe one accepted mode until the subscribing surface releases its lifetime. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Register the opt-in setting separately from Fluent node-card presentation. */
    setup(app) {
        if (this.#started)
            return;
        const ui = isRecord(app) && isRecord(app.ui) ? app.ui : null;
        const settings = ui && isRecord(ui.settings) ? ui.settings : null;
        if (!settings ||
            typeof settings.addSetting !== 'function' ||
            typeof settings.getSettingValue !== 'function')
            throw new Error('Comfy settings are unavailable for Substitute presentation.');
        this.#started = true;
        settings.addSetting.call(settings, {
            id: SUBSTITUTE_FRAME_SETTING,
            name: 'Substitute frame',
            ...sugarCubesSettingsSection('appearance', 'SubstituteFrame'),
            type: 'boolean',
            defaultValue: false,
            tooltip: 'Use Substitute workflow tabs, application orb and transparent Cube stack; hide the Comfy sidebar.',
            onChange: (value) => this.setEnabled(value === true),
        });
        this.setEnabled(settings.getSettingValue.call(settings, SUBSTITUTE_FRAME_SETTING, false) === true);
    }
    /** Release observers while leaving the persisted preference with Comfy. */
    dispose() {
        this.#listeners.clear();
    }
}
