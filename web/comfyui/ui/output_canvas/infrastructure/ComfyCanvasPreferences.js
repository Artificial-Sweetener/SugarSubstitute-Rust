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
/** Persist accepted canvas presentation choices through Comfy settings, independently of workflow JSON. */
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import { isRecord } from '../../types/common.js';
/** Identify the host's visibility preference for the output canvas. */
export const OUTPUT_CANVAS_VISIBLE_SETTING = 'SugarCubes.OutputCanvas.Visible';
/** Identify the host's accepted CSS width, with zero selecting automatic allocation. */
export const OUTPUT_CANVAS_WIDTH_SETTING = 'SugarCubes.OutputCanvas.Width';
/** Validate dynamic setting values once before they enter the allocation policy. */
export class ComfyCanvasPreferences {
    app;
    #listeners = new Set();
    #snapshot = Object.freeze({ visible: true, width: 0 });
    #write = null;
    #started = false;
    #disposed = false;
    /** Bind the host without reading persistence until extension setup is ready. */
    constructor(app) {
        this.app = app;
    }
    /** Read immutable accepted choices without storing image or workflow metadata. */
    get snapshot() {
        return this.#snapshot;
    }
    /** Subscribe to accepted local or external setting changes with consumer-owned cleanup. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Register two public presentation settings and restore only validated initial values. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        const app = isRecord(this.app) ? this.app : {};
        const ui = isRecord(app.ui) ? app.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : {};
        const { addSetting, getSettingValue, setSettingValueAsync } = settings;
        if (typeof addSetting !== 'function' ||
            typeof getSettingValue !== 'function' ||
            typeof setSettingValueAsync !== 'function')
            throw new Error('Comfy output canvas settings are unavailable.');
        this.#started = true;
        this.#write = async (id, value) => {
            await setSettingValueAsync.call(settings, id, value);
        };
        addSetting.call(settings, {
            id: OUTPUT_CANVAS_VISIBLE_SETTING,
            name: 'Show output canvas',
            ...sugarCubesSettingsSection('canvas', 'Visible'),
            type: 'boolean',
            defaultValue: true,
            onChange: (value) => this.#accept('visible', value),
        });
        addSetting.call(settings, {
            id: OUTPUT_CANVAS_WIDTH_SETTING,
            name: 'Output canvas width',
            ...sugarCubesSettingsSection('canvas', 'Width'),
            type: 'number',
            defaultValue: 0,
            tooltip: 'Width in CSS pixels. Use 0 for automatic allocation. The panel keeps space available for workflow controls.',
            attrs: { min: 0, max: 16384, step: 1 },
            onChange: (value) => this.#accept('width', value),
        });
        this.#accept('visible', getSettingValue.call(settings, OUTPUT_CANVAS_VISIBLE_SETTING, true));
        this.#accept('width', getSettingValue.call(settings, OUTPUT_CANVAS_WIDTH_SETTING, 0));
    }
    /** Await host persistence while the setting's onChange callback owns accepted visibility. */
    async visible(visible) {
        await this.#store(OUTPUT_CANVAS_VISIBLE_SETTING, visible);
    }
    /** Persist one completed resize without recording pointer movement or graph undo entries. */
    async width(width) {
        if (!Number.isFinite(width) || width < 0 || width > 16384)
            throw new RangeError('Output canvas width is invalid.');
        await this.#store(OUTPUT_CANVAS_WIDTH_SETTING, width);
    }
    /** Retire host-retained callbacks and ignore persistence completions after teardown. */
    dispose() {
        this.#disposed = true;
        this.#write = null;
        this.#listeners.clear();
    }
    /** Preserve storage failures for product feedback; Comfy owns its local setting and rollback policy. */
    async #store(id, value) {
        if (!this.#write || this.#disposed)
            throw new Error('Output canvas preferences are unavailable.');
        await this.#write(id, value);
    }
    /** Narrow untrusted host values and publish only actual accepted changes. */
    #accept(key, value) {
        if (this.#disposed || this.#snapshot[key] === value)
            return;
        if (key === 'visible' && typeof value === 'boolean')
            this.#snapshot = Object.freeze({ ...this.#snapshot, visible: value });
        else if (key === 'width' &&
            typeof value === 'number' &&
            Number.isFinite(value) &&
            value >= 0 &&
            value <= 16384)
            this.#snapshot = Object.freeze({ ...this.#snapshot, width: value });
        else
            return;
        for (const listener of this.#listeners)
            listener();
    }
}
