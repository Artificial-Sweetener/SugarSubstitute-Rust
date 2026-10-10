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
/** Persist stack presentation through Comfy settings, independently of workflow documents. */
import { isRecord } from '../../types/common.js';
/** Identify the persisted rail visibility affordance in Comfy settings. */
export const CUBE_STACK_VISIBLE_SETTING = 'SugarCubes.Masonry.CubeStack';
/** Identify the persisted expanded or compact rail preference. */
export const CUBE_STACK_COMPACT_SETTING = 'SugarCubes.Masonry.CompactStack';
/** Register and publish the two stack preferences with one authoritative persistence boundary. */
export class CubeStackPreferences {
    #app;
    #listeners = new Set();
    #snapshot = Object.freeze({ visible: true, compact: false });
    #setValue = null;
    #started = false;
    #disposed = false;
    /** Bind the dynamic Comfy application without reading persistence before host setup. */
    constructor(app) {
        this.#app = app;
    }
    /** Expose accepted preferences without mutating workflow metadata. */
    get snapshot() {
        return this.#snapshot;
    }
    /** Observe accepted setting changes until the stack releases its own lifetime. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Register persisted settings and restore their validated initial values. */
    setup() {
        if (this.#started || this.#disposed)
            return;
        const app = isRecord(this.#app) ? this.#app : {};
        const ui = isRecord(app.ui) ? app.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : {};
        const { addSetting, getSettingValue, setSettingValue } = settings;
        if (typeof addSetting !== 'function' ||
            typeof getSettingValue !== 'function' ||
            typeof setSettingValue !== 'function')
            throw new Error('Comfy settings persistence is unavailable for the Cube stack.');
        this.#started = true;
        this.#setValue = async (id, value) => {
            await setSettingValue.call(settings, id, value);
        };
        const register = (id, name, key, defaultValue) => {
            addSetting.call(settings, {
                id,
                name,
                category: ['SugarCubes', 'Masonry', name],
                type: 'boolean',
                defaultValue,
                onChange: (value) => {
                    if (typeof value === 'boolean')
                        this.#accept(key, value);
                },
            });
            const value = getSettingValue.call(settings, id, defaultValue);
            this.#accept(key, typeof value === 'boolean' ? value : defaultValue);
        };
        register(CUBE_STACK_VISIBLE_SETTING, 'Show Cube stack', 'visible', true);
        register(CUBE_STACK_COMPACT_SETTING, 'Compact Cube stack', 'compact', false);
    }
    /** Persist a visibility choice, including recovery through the normal Comfy settings UI. */
    setVisible(visible) {
        return this.#write(CUBE_STACK_VISIBLE_SETTING, 'visible', visible);
    }
    /** Persist expanded or compact presentation without recording an undoable graph edit. */
    setCompact(compact) {
        return this.#write(CUBE_STACK_COMPACT_SETTING, 'compact', compact);
    }
    /** Retire callbacks retained by host registration after the owning extension is disposed. */
    dispose() {
        this.#disposed = true;
        this.#setValue = null;
        this.#listeners.clear();
    }
    /** Accept only successful persistence and avoid publishing a stale result after teardown. */
    async #write(id, key, value) {
        if (!this.#setValue || this.#disposed)
            throw new Error('Cube stack preferences are unavailable.');
        await this.#setValue(id, value);
        this.#accept(key, value);
    }
    /** Publish one immutable accepted snapshot for either external settings or local controls. */
    #accept(key, value) {
        if (this.#disposed || this.#snapshot[key] === value)
            return;
        this.#snapshot = Object.freeze({ ...this.#snapshot, [key]: value });
        for (const listener of this.#listeners)
            listener();
    }
}
