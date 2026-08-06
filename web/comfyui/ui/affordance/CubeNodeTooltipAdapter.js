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
/** Replace generated Subgraph tooltip text on marked Cube instances. */
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeDefinitionDescription } from '../cube/node/CubeDefinitionIdentityWriter.js';
/** Adapt renderer-mounted tooltips while native definitions retain structural identity. */
export class CubeNodeTooltipAdapter {
    #document;
    #nodes;
    #observer;
    #originalText = new Map();
    #unsubscribe;
    #scheduled = false;
    /** Bind stable Nodes 2.0 header ids and live Cube metadata. */
    constructor(options) {
        this.#document = options.document;
        this.#nodes = options.nodes;
        this.#observer = new MutationObserver(() => this.#schedule());
        this.#unsubscribe = options.nodes.subscribe(() => this.#schedule());
    }
    /** Observe lazy PrimeVue tooltip mounts and reconcile current Cube headers. */
    install() {
        this.#observer.observe(this.#document.body, { childList: true, subtree: true });
        this.refresh();
    }
    /** Restore host tooltip text when the graph-bound runtime is replaced. */
    dispose() {
        this.#observer.disconnect();
        this.#unsubscribe();
        for (const [element, text] of this.#originalText)
            element.textContent = text;
        this.#originalText.clear();
    }
    /** Replace only tooltips attached to a catalogued Cube node header. */
    refresh() {
        this.#scheduled = false;
        for (const node of this.#nodes.list())
            this.#adaptNode(node);
    }
    /** Coalesce lazy tooltip and renderer mount bursts. */
    #schedule() {
        if (this.#scheduled)
            return;
        this.#scheduled = true;
        queueMicrotask(() => this.refresh());
    }
    /** Follow the header-to-tooltip accessibility relationship without reading translated text. */
    #adaptNode(node) {
        const header = this.#document.querySelector(`[data-testid="node-header-${cssEscape(String(node.id))}"]`);
        const title = header?.querySelector('[data-testid="node-title"]');
        const tooltipId = title?.getAttribute('aria-describedby');
        const text = tooltipId
            ? this.#document.getElementById(tooltipId)?.querySelector('.p-tooltip-text')
            : null;
        if (!text)
            return;
        if (!this.#originalText.has(text))
            this.#originalText.set(text, text.textContent ?? '');
        const description = resolveCubeDefinitionDescription(requireCubeIdentity(node));
        if (text.textContent !== description)
            text.textContent = description;
    }
}
/** Escape generated graph ids without relying on browser-specific CSS globals. */
function cssEscape(value) {
    return value.replaceAll('\\', '\\\\').replaceAll('"', '\\"');
}
