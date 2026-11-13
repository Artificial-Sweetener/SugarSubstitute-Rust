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
/** Store the desktop application's memory ceiling in Comfy's persisted settings. */
import { isRecord } from '../types/common.js';
import { sugarCubesSettingsSection } from './SugarCubesSettingsSections.js';
const MODE_ID = 'SugarCubes.Memory.Mode';
const SHARE_ID = 'SugarCubes.Memory.SharePercent';
const FIXED_ID = 'SugarCubes.Memory.FixedGiB';
const SHARE_MODE = 'Share of system memory';
const FIXED_MODE = 'Fixed size';
const DEFAULT_SHARE = 25;
const DEFAULT_FIXED_GIB = 8;
const GIB = 1024 * 1024 * 1024;
/** Register the memory settings once and report the preference they select. */
export class MemoryBudgetSettings {
    #listeners = new Set();
    #settings;
    /** Values delivered by change callbacks, which can precede Comfy's own store update. */
    #changed = new Map();
    /** Register the three settings rows in Comfy's settings dialog. */
    constructor(app) {
        const host = isRecord(app) ? app : {};
        const ui = isRecord(host.ui) ? host.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : null;
        if (!settings || typeof settings.addSetting !== 'function')
            throw new Error('Comfy settings are unavailable for the memory budget.');
        this.#settings = settings;
        const add = (definition) => {
            settings.addSetting.call(settings, {
                ...definition,
                onChange: (value) => {
                    this.#changed.set(definition.id, value);
                    const preference = this.read();
                    for (const listener of [...this.#listeners])
                        listener(preference);
                },
            });
        };
        add({
            id: MODE_ID,
            name: 'Application memory limit',
            ...sugarCubesSettingsSection('memory', 'Mode'),
            type: 'combo',
            options: [SHARE_MODE, FIXED_MODE],
            defaultValue: SHARE_MODE,
            tooltip: 'How much memory SugarSubstitute may use for its own work and caches. Caches shrink at once when the system needs memory back.',
        });
        add({
            id: SHARE_ID,
            name: 'Share of system memory (%)',
            ...sugarCubesSettingsSection('memory', 'SharePercent'),
            type: 'slider',
            attrs: { min: 10, max: 75, step: 5 },
            defaultValue: DEFAULT_SHARE,
        });
        add({
            id: FIXED_ID,
            name: 'Fixed size (GB)',
            ...sugarCubesSettingsSection('memory', 'FixedGiB'),
            type: 'number',
            attrs: { min: 1, max: 1024, step: 1 },
            defaultValue: DEFAULT_FIXED_GIB,
        });
    }
    /** The preference the stored settings select, with out-of-range values clamped. */
    read() {
        if (this.#value(MODE_ID) === FIXED_MODE) {
            const gib = Number(this.#value(FIXED_ID) ?? DEFAULT_FIXED_GIB);
            const clamped = Number.isFinite(gib) ? Math.min(1024, Math.max(1, Math.round(gib))) : 8;
            return { mode: 'fixed', bytes: clamped * GIB };
        }
        const share = Number(this.#value(SHARE_ID) ?? DEFAULT_SHARE);
        const percent = Number.isFinite(share) ? Math.min(75, Math.max(10, Math.round(share))) : 25;
        return { mode: 'share', percent };
    }
    /** Observe preference changes until the returned release is called. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    #value(id) {
        if (this.#changed.has(id))
            return this.#changed.get(id);
        const get = this.#settings.getSettingValue;
        return typeof get === 'function'
            ? get.call(this.#settings, id)
            : undefined;
    }
}
