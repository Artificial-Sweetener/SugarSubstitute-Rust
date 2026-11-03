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
/** Relate ordered mask loaders to the regional prompts that name and count their regions. */
import { regionMaskColor } from '../domain/MaskColorScheme.js';
/**
 * Read regional relations from the same graph facts that plan input surfaces.
 *
 * Relations are read lazily and cached until the graph reports a change, so every card row and
 * hover between two changes sees one consistent snapshot without repeating topology walks.
 */
export class WorkflowRegionalPrompts {
    graph;
    report;
    #listeners = new Set();
    #stop;
    #masks = null;
    /** Observe the graph facts owner. */
    constructor(graph, report) {
        this.graph = graph;
        this.report = report;
        this.#stop = graph.subscribe(() => {
            this.#masks = null;
            for (const listener of this.#listeners)
                try {
                    listener();
                }
                catch (error) {
                    this.report(error);
                }
        });
    }
    /** Every ordered mask loader whose MASK output is used, in graph order; none when unreadable. */
    masks() {
        if (this.#masks)
            return this.#masks;
        let masks = [];
        try {
            masks = this.graph.facts().masks.flatMap((mask) => mask.regions
                ? [
                    {
                        endpoint: mask.key,
                        entries: mask.entries.flatMap((entry) => entry.entry === null ? [] : [entry.entry]),
                        names: mask.entries.map((_, index) => mask.regions?.names[index] ?? null),
                        prompts: mask.regions.prompts,
                    },
                ]
                : []);
        }
        catch (error) {
            this.report(error);
        }
        this.#masks = masks;
        return masks;
    }
    /** The ordered mask loader with this endpoint, or null. */
    mask(endpoint) {
        return this.masks().find((mask) => mask.endpoint === endpoint) ?? null;
    }
    /** Every ordered mask loader related to the prompt at this workflow path. */
    forPrompt(prompt) {
        return this.masks().filter((mask) => mask.prompts.includes(prompt));
    }
    /** Name and colour one region, or null when the entry is not a region of an ordered loader. */
    region(endpoint, entry) {
        const mask = this.mask(endpoint), index = mask ? mask.entries.indexOf(entry) : -1;
        if (!mask || index < 0)
            return null;
        return {
            label: mask.names[index] ?? null,
            color: regionMaskColor(index, mask.entries.length),
        };
    }
    /** Observe relation changes; read them again on notification. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Stop observing the graph. */
    dispose() {
        this.#stop();
        this.#listeners.clear();
        this.#masks = null;
    }
}
