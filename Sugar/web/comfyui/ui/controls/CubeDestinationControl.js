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
/** Own concrete Cube destination selection and in-session pack creation. */
import { ComfySettingsSingleSelectControl, } from './ComfySettingsSelect.js';
/** Keep destination presentation and asynchronous pack insertion out of the authoring modal. */
export class CubeDestinationControl {
    element;
    #control;
    #locked;
    #onChange;
    #onCreateDestination;
    #onError;
    #entries;
    #selected;
    constructor(options) {
        this.#locked = options.locked;
        this.#onChange = options.onChange;
        this.#onCreateDestination = options.onCreateDestination;
        this.#onError = options.onError;
        this.#entries = normalizeDestinations(options.destinations, options.candidateDestination);
        this.#selected = selectInitialDestination(this.#entries, options.candidateDestination);
        this.#control = new ComfySettingsSingleSelectControl(options.documentRef, options.renderer, {
            ariaLabel: 'Save to',
            disabled: this.#locked,
            options: destinationOptions(this.#entries),
            value: this.#selected?.key ?? '',
            onChange: (value) => {
                void this.#handleChange(value);
            },
        });
        this.element = this.#control.element;
        this.element.classList.add('sugarcubes-create-cube__destination-select');
    }
    /** Release Comfy's mounted Settings control. */
    dispose() {
        this.#control.dispose();
    }
    /** Return the selected entry key for presentation-level tests and orchestration. */
    selectedKey() {
        return this.#selected?.key ?? '';
    }
    /** Select one entry through the same application path used by the native component. */
    select(key) {
        void this.#handleChange(key);
    }
    /** Return the concrete persistence destination currently selected by the author. */
    destination() {
        return this.#selected?.destination ?? null;
    }
    /** Return a human-readable destination for the identity preview. */
    presentation() {
        const selected = this.#selected;
        if (!selected)
            return 'Destination required';
        return selected.detail ? `${selected.label} — ${selected.detail}` : selected.label;
    }
    /** Resolve regular selection or insert a newly created pack without closing the modal. */
    async #handleChange(key) {
        const requested = this.#entries.find((entry) => entry.key === key);
        if (!requested)
            return;
        if (requested.action !== 'create-pack') {
            this.#select(requested);
            return;
        }
        const previous = this.#selected;
        this.#control.update({ disabled: true, value: requested.key });
        try {
            const created = await this.#onCreateDestination?.();
            if (!created?.destination) {
                this.#selected = previous;
                this.#render();
                return;
            }
            if (!this.#entries.some((entry) => entry.key === created.key)) {
                const actionIndex = this.#entries.findIndex((entry) => entry.action);
                const insertAt = actionIndex >= 0 ? actionIndex : this.#entries.length;
                this.#entries.splice(insertAt, 0, created);
            }
            this.#select(created);
        }
        catch (error) {
            this.#selected = previous;
            this.#render();
            this.#onError(error instanceof Error ? error.message : 'Failed to create Cube Pack.');
        }
        finally {
            this.#control.update({ disabled: this.#locked });
        }
    }
    /** Select one concrete entry and notify the modal's derived-value owner. */
    #select(entry) {
        if (!entry.destination)
            return;
        this.#selected = entry;
        this.#render();
        this.#onChange();
    }
    /** Render native Settings options from the authoritative destination catalog. */
    #render() {
        this.#control.update({
            disabled: this.#locked,
            options: destinationOptions(this.#entries),
            value: this.#selected?.key ?? '',
        });
    }
}
/** Convert destination entries into Comfy's Settings option contract. */
function destinationOptions(entries) {
    return entries.map((entry) => ({
        label: entry.detail ? `${entry.label} (${entry.detail})` : entry.label,
        value: entry.key,
    }));
}
/** Supply a concrete local fallback when a caller has no destination catalog. */
function normalizeDestinations(destinations, candidateDestination) {
    if (destinations.length)
        return [...destinations];
    const destination = readConcreteDestination(candidateDestination) ?? { kind: 'local' };
    return [
        destination.kind === 'local'
            ? {
                key: 'local/personal',
                label: 'Personal cubes',
                detail: 'Saved locally',
                destination,
            }
            : {
                key: `pack/${destination.repoRef}`,
                label: destination.repo,
                detail: destination.owner,
                destination,
            },
    ];
}
/** Select the requested concrete destination, requested kind, or first writable entry. */
function selectInitialDestination(destinations, candidateDestination) {
    const requested = readConcreteDestination(candidateDestination);
    if (requested) {
        const matching = destinations.find((entry) => entry.destination?.kind === requested.kind &&
            (requested.kind === 'local' ||
                (entry.destination?.kind === 'pack' && entry.destination.repoRef === requested.repoRef)));
        if (matching)
            return matching;
    }
    if (candidateDestination === 'pack') {
        const pack = destinations.find((entry) => entry.destination?.kind === 'pack');
        if (pack)
            return pack;
    }
    return destinations.find((entry) => Boolean(entry.destination)) ?? null;
}
/** Narrow an authoring draft destination into a concrete persistence target. */
function readConcreteDestination(destination) {
    return typeof destination === 'object' && destination ? destination : null;
}
