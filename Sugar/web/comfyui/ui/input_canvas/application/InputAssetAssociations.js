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
/** Publish reusable graph-derived loader relationships independently of any preview or canvas implementation. */
import { resolveInputAssetAssociations, exclusiveInputImage, } from '../domain/InputAssetAssociation.js';
/** Own relationship queries and scoped changes for every application consumer. */
export class InputAssetAssociations {
    #associations = new Map();
    #maskSources = new Set();
    #listeners = new Set();
    /** Return an explicit missing relationship for stale or undiscovered field identities. */
    read(mask) {
        return this.#associations.get(mask) ?? { kind: 'missing', mask, candidates: [], consumers: [] };
    }
    /** Query reverse ownership without asking another consumer to infer relationships. */
    masksFor(image) {
        return [...this.#associations.values()]
            .flatMap((association) => association.kind === 'associated' && association.image === image ? [association.mask] : [])
            .sort();
    }
    /** Resolve exclusive input ownership through the same policy used by execution preparation. */
    exclusiveImage(mask) {
        return exclusiveInputImage(this.#associations, mask, this.#maskSources);
    }
    /** Freeze exclusive logical pairs for execution without re-reading any native graph or field values. */
    exclusivePairs() {
        return new Map([...this.#associations.keys()].flatMap((mask) => {
            const image = this.exclusiveImage(mask);
            return image ? [[mask, image]] : [];
        }));
    }
    /** Replace normalized graph facts atomically and preserve unchanged association identities. */
    replace(graphs) {
        const next = new Map(resolveInputAssetAssociations(graphs)), changed = new Set();
        for (const mask of new Set([...next.keys(), ...this.#associations.keys()])) {
            const previous = this.#associations.get(mask), current = next.get(mask);
            if (JSON.stringify(previous) !== JSON.stringify(current))
                changed.add(mask);
            else if (previous)
                next.set(mask, previous);
        }
        const maskSources = new Set(graphs.flatMap((graph) => graph.endpoints.flatMap((endpoint) => (endpoint.maskOutput ? [endpoint.key] : []))));
        const images = new Set([...maskSources, ...this.#maskSources].filter((image) => maskSources.has(image) !== this.#maskSources.has(image)));
        for (const mask of changed)
            for (const item of [this.#associations.get(mask), next.get(mask)])
                if (item?.kind === 'associated')
                    images.add(item.image);
                else
                    for (const image of item?.candidates ?? [])
                        images.add(image);
        for (const item of [...this.#associations.values(), ...next.values()])
            if (item.kind === 'associated'
                ? images.has(item.image)
                : item.candidates.some((image) => images.has(image)))
                changed.add(item.mask);
        this.#associations = next;
        this.#maskSources = maskSources;
        if (changed.size)
            for (const listener of this.#listeners)
                listener(changed);
    }
    /** Subscribe to identified relationship changes without exposing mutable graph or preview state. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
}
