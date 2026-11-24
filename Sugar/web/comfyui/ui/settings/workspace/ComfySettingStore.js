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
 * ComfyUI's setting store through its supported surface: values through
 * `app.extensionManager.setting` (`get`, `set`, `settings`) and changes through the
 * `<id>.change` events ComfyUI dispatches on `app.ui.settings` for every setting.
 */
import { isRecord } from '../../types/common.js';
/** The store, or an explanation of what in ComfyUI changed. */
export class ComfySettingStore {
    app;
    constructor(app) {
        this.app = app;
    }
    /** Read one setting's current value. */
    get(id) {
        return this.#api().get(id);
    }
    /** Write one setting through ComfyUI, which validates, persists and runs its handlers. */
    set(id, value) {
        return Promise.resolve(this.#api().set(id, value)).then(() => undefined);
    }
    /** Every registered setting, in registration order. */
    definitions() {
        const settings = this.#api().settings;
        return Object.values(settings).filter((value) => isRecord(value) && typeof value.id === 'string');
    }
    /** Follow one setting's changes, whoever makes them. */
    subscribe(id, listener) {
        const target = this.#events();
        const handler = () => listener();
        target.addEventListener(`${id}.change`, handler);
        return () => target.removeEventListener(`${id}.change`, handler);
    }
    /**
     * Register a setting. SugarCubes' settings belong to feature instances created after ComfyUI
     * registers the extension, so they register through `app.ui.settings.addSetting` rather than
     * the extension's static `settings` list. Values the workspace draws register as `hidden`:
     * ComfyUI keeps them, persists them and runs their handlers, and never draws them.
     */
    register(definition) {
        const ui = isRecord(this.app) ? this.app.ui : null;
        const settings = isRecord(ui) ? ui.settings : null;
        if (!isRecord(settings) || typeof settings.addSetting !== 'function')
            throw new Error('ComfyUI settings changed: settings registration is unavailable.');
        settings.addSetting.call(settings, definition);
    }
    /** A live value bound to one setting. */
    value(id, decode) {
        return {
            read: () => decode(this.get(id)),
            write: (value) => this.set(id, value),
            subscribe: (listener) => this.subscribe(id, listener),
        };
    }
    #api() {
        const manager = isRecord(this.app) ? this.app.extensionManager : null;
        const setting = isRecord(manager) ? manager.setting : null;
        if (!isRecord(setting) ||
            typeof setting.get !== 'function' ||
            typeof setting.set !== 'function' ||
            !isRecord(setting.settings))
            throw new Error('ComfyUI settings changed: extensionManager.setting is unavailable.');
        const api = setting;
        const get = setting.get;
        const set = setting.set;
        const settings = setting.settings;
        return {
            get: (id) => get.call(api, id),
            set: (id, value) => set.call(api, id, value),
            settings,
        };
    }
    #events() {
        const ui = isRecord(this.app) ? this.app.ui : null;
        const settings = isRecord(ui) ? ui.settings : null;
        if (!(settings instanceof EventTarget))
            throw new Error('ComfyUI settings changed: setting change events are unavailable.');
        return settings;
    }
}
/** A boolean read leniently from a stored value. */
export const asBoolean = (value) => value === true;
/** A string read from a stored value, or empty. */
export const asString = (value) => (typeof value === 'string' ? value : '');
/** A finite number read from a stored value, or zero. */
export const asNumber = (value) => typeof value === 'number' && Number.isFinite(value) ? value : 0;
