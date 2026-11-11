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
/** Own cached preset queries and serialized durable mutations independently of menus. */
import { canonicalDimensions } from '../domain/DimensionPolicy.js';
import { encodePresetDocument } from '../domain/PresetCodec.js';
import { sameAssociation } from '../domain/UserPreset.js';
/** Publish snapshots only after persistence succeeds; menu opens consume prepared values only. */
export class UserPresetLibrary {
    #repository;
    #id;
    #clock;
    #records = null;
    #loading = null;
    #queue = Promise.resolve();
    /** Inject persistence, identity and time to keep application behavior deterministic. */
    constructor(repository, id, clock) {
        this.#repository = repository;
        this.#id = id;
        this.#clock = clock;
    }
    /** Prepare storage away from context-menu hot paths and share one in-flight read. */
    prepare() {
        if (this.#records)
            return Promise.resolve();
        return (this.#loading ??= this.#repository
            .load()
            .then((records) => {
            this.#records = encodePresetDocument(records).presets;
        })
            .finally(() => {
            this.#loading = null;
        }));
    }
    /** Return detached records so consumers cannot change authoritative cached data. */
    current() {
        return encodePresetDocument(this.#records ?? []).presets;
    }
    /** Select dimension shapes in Substitute's size, label and identity order. */
    dimensions(association) {
        return this.current()
            .filter((preset) => preset.kind === 'dimension' &&
            preset.associations.some((target) => sameAssociation(target, association)))
            .sort((a, b) => a.payload.short_edge - b.payload.short_edge ||
            a.payload.long_edge - b.payload.long_edge ||
            compareText(a.label, b.label) ||
            compareText(a.id, b.id));
    }
    /** Return each node preset once, preferring the supplied most-specific scopes. */
    nodePresets(nodeType, associations) {
        const eligible = this.current()
            .filter((preset) => preset.kind === 'node_inputs' && preset.payload.node_type === nodeType)
            .sort((a, b) => compareText(a.label, b.label) || compareText(a.id, b.id));
        const seen = new Set();
        return associations.flatMap((association) => eligible.filter((preset) => {
            if (seen.has(preset.id) ||
                !preset.associations.some((target) => sameAssociation(target, association)))
                return false;
            seen.add(preset.id);
            return true;
        }));
    }
    /** Merge equal canonical shapes across scopes while retaining identity and creation time. */
    saveDimensions(width, height, association) {
        const payload = canonicalDimensions(width, height);
        if (!payload)
            return Promise.reject(new Error('Dimensions must be positive integers.'));
        return this.#mutate((records) => {
            const index = records.findIndex((preset) => preset.kind === 'dimension' &&
                preset.payload.short_edge === payload.short_edge &&
                preset.payload.long_edge === payload.long_edge);
            const existing = records[index];
            if (existing) {
                if (!existing.associations.some((target) => sameAssociation(target, association)))
                    records[index] = {
                        ...existing,
                        associations: [...existing.associations, association],
                        updated_at: this.#clock(),
                    };
            }
            else
                records.push({
                    ...this.#record('dimension', `${payload.short_edge} x ${payload.long_edge}`, association),
                    kind: 'dimension',
                    payload,
                });
        });
    }
    /** Update a named node preset in its exact scope using detached validated JSON inputs. */
    saveNode(label, nodeType, inputs, association) {
        const name = label.trim();
        if (!name)
            return Promise.reject(new Error('Preset name is required.'));
        return this.#mutate((records) => {
            const index = records.findIndex((preset) => preset.kind === 'node_inputs' &&
                preset.payload.node_type === nodeType &&
                preset.label.toLowerCase() === name.toLowerCase() &&
                preset.associations.some((target) => sameAssociation(target, association)));
            const existing = records[index];
            const preset = {
                ...(existing ?? this.#record('node_inputs', name, association)),
                label: name,
                updated_at: this.#clock(),
                kind: 'node_inputs',
                payload: { node_type: nodeType, inputs },
            };
            if (existing)
                records[index] = preset;
            else
                records.push(preset);
        });
    }
    /** Serialize whole-document updates and preserve the old snapshot on failed writes. */
    #mutate(change) {
        const operation = this.#queue.then(async () => {
            await this.prepare();
            const records = [...this.current()];
            change(records);
            const validated = encodePresetDocument(records).presets;
            await this.#repository.save(validated);
            this.#records = validated;
        });
        this.#queue = operation.then(() => undefined, () => undefined);
        return operation;
    }
    /** Generate compatible stable identities and UTC timestamps for new records. */
    #record(kind, label, association) {
        const timestamp = this.#clock();
        return {
            id: `${kind}:${this.#id()}`,
            label,
            associations: [association],
            created_at: timestamp,
            updated_at: timestamp,
        };
    }
}
/** Sort deterministically without browser locale changing library ordering. */
function compareText(left, right) {
    const a = left.toLowerCase(), b = right.toLowerCase();
    return a < b ? -1 : a > b ? 1 : 0;
}
