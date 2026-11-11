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
/** Adapt cached ordinary output handles to fenced, undoable workflow metadata edits. */
import { createComfyCubeHistoryAdapter } from '../cube/ComfyCubeHistoryAdapter.js';
import { isRecord } from '../types/common.js';
import { encodeSourceSavingPreferences, readWorkflowSourceSavingPreferences, } from './domain/SourceSavingPreferences.js';
/** Preserve graph layout and widget values while storing preferences by stable execution path. */
export class ComfySourceSavingDocument {
    options;
    /** Bind existing topology and history owners; never discover nodes from a popup mount. */
    constructor(options) {
        this.options = options;
    }
    /** Capture the active scope before asynchronous UI work and merge deliberately changed sources. */
    begin() {
        const root = this.options.root(), canvas = this.options.canvas(), scope = this.options.scope();
        if (!isRecord(root) || !isRecord(canvas) || scope === null || scope === undefined)
            throw new Error('The active workflow is unavailable.');
        const preferences = readWorkflowSourceSavingPreferences(root);
        const sources = this.options
            .sources()
            .filter((source) => typeof source.node.type === 'string' && this.options.capabilities().has(source.node.type));
        const targets = sources.map((source) => {
            const type = String(source.node.type), capability = this.options.capabilities().get(type);
            const properties = isRecord(source.node.properties) ? source.node.properties : {};
            const legacyLabel = typeof properties.sugarcubes_output_label === 'string'
                ? properties.sugarcubes_output_label.trim() || null
                : null;
            return Object.freeze({
                path: source.path,
                title: source.label,
                classType: type,
                supported: capability?.managed === true,
                kind: capability?.kind ?? null,
                preference: preferences.get(source.path) ?? {
                    enabled: properties.sugarcubes_save_outputs !== false,
                    label: legacyLabel,
                },
            });
        });
        const current = () => {
            if (this.options.root() !== root || this.options.scope() !== scope)
                return false;
            const active = new Map(this.options.sources().map((source) => [source.path, source.node]));
            return sources.every((source) => active.get(source.path) === source.node);
        };
        return {
            targets: Object.freeze(targets),
            current,
            commit: (values) => {
                if (!current())
                    throw new Error('The workflow changed while output settings were open.');
                const changes = targets.filter((target) => {
                    const next = values.get(target.path);
                    if (!next)
                        throw new Error('An output source choice is missing.');
                    return (next.enabled !== target.preference.enabled || next.label !== target.preference.label);
                });
                if (!changes.length)
                    return;
                const updated = new Map(readWorkflowSourceSavingPreferences(root));
                for (const target of changes) {
                    const value = values.get(target.path);
                    if (!value)
                        throw new Error('An output source choice is missing.');
                    if (!target.supported && value.enabled !== target.preference.enabled)
                        throw new Error('This saver retains its native saving controls.');
                    updated.set(target.path, value);
                }
                const prior = root.extra;
                const extra = isRecord(prior) ? prior : {};
                const header = isRecord(extra.sugarcubes_output_saving)
                    ? extra.sugarcubes_output_saving
                    : {};
                const nextExtra = {
                    ...extra,
                    sugarcubes_output_saving: {
                        ...header,
                        source_preferences: encodeSourceSavingPreferences(updated),
                    },
                };
                readWorkflowSourceSavingPreferences({ extra: nextExtra });
                const history = createComfyCubeHistoryAdapter({ graph: root, canvas });
                history.beforeChange();
                try {
                    root.extra = nextExtra;
                    history.setDirtyCanvas(true, true);
                }
                catch (error) {
                    if (current())
                        root.extra = prior;
                    throw error;
                }
                finally {
                    history.afterChange();
                }
            },
        };
    }
}
