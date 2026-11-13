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
/** Constrain the native Properties setting before host state and persistence commit. */
import { isRecord } from '../types/common.js';
const PANEL_SETTING = 'Comfy.RightSidePanel.IsOpen';
/** Lease the host's public setting normalizer without changing any panel opening callsite. */
export class ComfyPropertiesPanelAdapter {
    #setting;
    #reportError;
    #definition = null;
    #previous;
    #hadPrevious = false;
    #blocked = false;
    #savedOpen = null;
    /** Validate the public extension setting surface at the dynamic host boundary. */
    constructor(app, reportError) {
        const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : {};
        const setting = isRecord(manager.setting) ? manager.setting : null;
        this.#setting =
            setting && typeof setting.get === 'function' && typeof setting.set === 'function'
                ? setting
                : null;
        this.#reportError = reportError;
    }
    /** Install the public pre-commit hook once, retaining the exact native definition. */
    setup() {
        if (this.#definition || !this.#setting)
            return;
        const definitions = this.#setting.settings;
        const definition = isRecord(definitions) ? definitions[PANEL_SETTING] : undefined;
        if (!isRecord(definition))
            return;
        this.#definition = definition;
        this.#hadPrevious = Object.hasOwn(definition, 'migrateDeprecatedValue');
        this.#previous = definition.migrateDeprecatedValue;
        // Comfy invokes this public normalization hook before onChange, reactive state and persistence.
        definition.migrateDeprecatedValue = this.#normalize;
    }
    /** Save graph state on entry, constrain masonry, and restore state after departure. */
    update(blocked) {
        const entering = blocked && !this.#blocked;
        this.#blocked = blocked;
        if (!blocked) {
            this.#restore();
            return true;
        }
        this.setup();
        if (!this.#setting)
            return true;
        try {
            if (!this.#definition)
                throw new Error('Comfy Properties setting definition is unavailable.');
            const get = this.#setting.get;
            const set = this.#setting.set;
            if (typeof get !== 'function' || typeof set !== 'function')
                throw new Error('Comfy Properties setting access is unavailable.');
            const open = get.call(this.#setting, PANEL_SETTING);
            if (typeof open !== 'boolean')
                throw new Error('Comfy Properties setting is not boolean.');
            if (entering)
                this.#savedOpen = open;
            if (open) {
                const result = set.call(this.#setting, PANEL_SETTING, false);
                void Promise.resolve(result).catch(this.#reportError);
            }
            if (get.call(this.#setting, PANEL_SETTING) !== false)
                throw new Error('Comfy Properties panel did not close before masonry preparation.');
            return true;
        }
        catch (error) {
            this.#reportError(error);
            return false;
        }
    }
    /** Restore the saved graph state and exact host normalizer on teardown. */
    dispose() {
        this.update(false);
        const definition = this.#definition;
        if (definition?.migrateDeprecatedValue === this.#normalize) {
            if (this.#hadPrevious)
                definition.migrateDeprecatedValue = this.#previous;
            else
                delete definition.migrateDeprecatedValue;
        }
        this.#definition = null;
        this.#previous = undefined;
    }
    /** Consume the saved state once, reporting native restoration failures without hiding graph. */
    #restore() {
        const open = this.#savedOpen;
        this.#savedOpen = null;
        if (open === null || !this.#setting)
            return;
        try {
            const get = this.#setting.get;
            const set = this.#setting.set;
            if (typeof get !== 'function' || typeof set !== 'function')
                throw new Error('Comfy Properties setting access is unavailable during restoration.');
            if (get.call(this.#setting, PANEL_SETTING) === open)
                return;
            const result = set.call(this.#setting, PANEL_SETTING, open);
            void Promise.resolve(result).catch(this.#reportError);
        }
        catch (error) {
            this.#reportError(error);
        }
    }
    /** Preserve native normalization in graph view and return only closed state while blocked. */
    #normalize = (value) => {
        if (this.#blocked)
            return false;
        return typeof this.#previous === 'function'
            ? this.#previous.call(this.#definition, value)
            : value;
    };
}
