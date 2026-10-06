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
/** Own editor menu entry-point lifetimes while delegating values, policies and persistence. */
import { ContextMenu } from '../../menus/ContextMenu.js';
import { inferDimensionPairs } from '../domain/DimensionPolicy.js';
import { readNativeEditorCard, projectedEditorCardMount, } from '../infrastructure/NativeEditorCardAdapter.js';
import { SaveNodePresetDialog } from './SaveNodePresetDialog.js';
import { nodeActionEntries } from './NodeActionMenu.js';
import { dimensionMenuEntries } from './DimensionMenu.js';
import { dimensionFieldTarget } from './DimensionFieldTarget.js';
/** Use one native-card owner for both SugarCube faces and Workflow Masonry. */
export class EditorContextMenuHost {
    #options;
    #menu;
    #dialog;
    #cards = new Map();
    #presence;
    #dirty = new Set();
    #queued = false;
    #active = false;
    #menuRoot = null;
    /** Prepare delegated owners without touching cards until setup completes. */
    constructor(options) {
        this.#options = options;
        this.#menu = new ContextMenu(options.document);
        this.#dialog = new SaveNodePresetDialog(options.document);
        this.#presence = new MutationObserver((records) => {
            for (const record of records) {
                if (record.target instanceof Element &&
                    record.target.closest('[data-sc-floating-surface], .sugarcubes-modal-overlay'))
                    continue;
                for (const node of record.addedNodes)
                    if (node instanceof HTMLElement)
                        this.#queue(node);
                if (record.target instanceof HTMLElement && record.target.closest('.lg-node'))
                    this.#queue(record.target);
            }
            this.#schedule();
        });
    }
    /** Observe native card mounts and intercept dimension menus before embedded event isolation. */
    setup() {
        if (this.#active)
            return;
        this.#active = true;
        this.#presence.observe(this.#options.document.body, { subtree: true, childList: true });
        this.#options.document.addEventListener('contextmenu', this.#context, true);
        this.refresh();
    }
    /** Follow renderer switches without retaining detached or unavailable native controls. */
    refresh() {
        this.#queue(this.#options.document.body);
        this.#schedule();
    }
    /** Expose prepared live card boundaries for context projection without rediscovering Vue trees. */
    preparedCards() {
        return [...this.#cards.values()].map((record) => record.card);
    }
    /** Remove every owned listener, menu and modal on shutdown. */
    dispose() {
        this.#active = false;
        this.#presence.disconnect();
        this.#options.document.removeEventListener('contextmenu', this.#context, true);
        this.#menu.close();
        this.#dialog.close();
        this.#cards.clear();
        this.#dirty.clear();
    }
    /** Collect only the nearest card and newly introduced native roots. */
    #queue(element) {
        const own = element.closest('.lg-node');
        if (own)
            this.#dirty.add(own);
        for (const root of element.querySelectorAll('.lg-node'))
            this.#dirty.add(root);
    }
    /** Coalesce structural observation; menu contents are rebuilt only on an explicit open gesture. */
    #schedule() {
        if (this.#queued || !this.#active)
            return;
        this.#queued = true;
        queueMicrotask(() => {
            this.#queued = false;
            if (!this.#active)
                return;
            try {
                const enabled = this.#options.enabled?.() ?? true;
                if (!enabled ||
                    (this.#menuRoot &&
                        (!this.#menuRoot.isConnected || !projectedEditorCardMount(this.#menuRoot)))) {
                    this.#menu.close();
                    this.#dialog.close();
                    this.#menuRoot = null;
                }
                for (const root of this.#cards.keys())
                    if (!enabled || !root.isConnected)
                        this.#cards.delete(root);
                if (enabled)
                    for (const root of this.#dirty)
                        if (root.isConnected)
                            this.#bind(root);
            }
            catch (error) {
                this.#options.reportError(error);
            }
            this.#dirty.clear();
        });
    }
    /** Retain recognized native cards without inserting presentation controls. */
    #bind(root) {
        const card = this.#read(root);
        if (!card?.header) {
            this.#cards.delete(root);
            return;
        }
        const existing = this.#cards.get(root);
        if (existing) {
            existing.card = card;
            return;
        }
        this.#cards.set(root, { root, card });
    }
    /** Resolve the card again at command entry so replaced controls never retain stale value bindings. */
    #read(root) {
        if (!projectedEditorCardMount(root))
            return null;
        return (this.#options.readCard ?? readNativeEditorCard)(root);
    }
    /** Compose available commands through their authoritative policy owners. */
    #entries(card) {
        return nodeActionEntries(card, this.#options.library, this.#options.families(card), this.#dialog, this.#run, this.#options.skipped, this.#options.modelContributions?.(card.root) ?? []);
    }
    /** Route exact numeric pair fields while retaining model and prompt-specific menu ownership. */
    #context = (event) => {
        if (!(event.target instanceof Element) || this.#options.enabled?.() === false)
            return;
        const root = event.target.closest('.lg-node');
        if (!root)
            return;
        const card = this.#read(root);
        if (!card)
            return;
        if (card.header?.contains(event.target)) {
            const entries = this.#entries(card);
            if (!entries.length)
                return;
            event.preventDefault();
            event.stopImmediatePropagation();
            this.#menuRoot = root;
            this.#menu.open({
                label: `${card.title} node actions`,
                entries,
                anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
                trigger: card.header,
            });
            return;
        }
        const field = dimensionFieldTarget(root, card.fields, event.target, event.clientX, event.clientY);
        const pair = field
            ? inferDimensionPairs(card.fields.map((field) => field.name)).find((pair) => pair.width === field.name || pair.height === field.name)
            : null;
        if (!pair || field?.kind !== 'number')
            return;
        const width = card.fields.find((field) => field.name === pair.width), height = card.fields.find((field) => field.name === pair.height);
        if (!width || !height || width.kind !== 'number' || height.kind !== 'number')
            return;
        const entries = dimensionMenuEntries({ width, height }, field.name === pair.width ? 'width' : 'height', this.#options.library, this.#options.families(card), this.#run);
        if (!entries.length)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        this.#menuRoot = root;
        this.#menu.open({
            label: 'Dimensions',
            entries,
            anchor: { kind: 'pointer', x: event.clientX, y: event.clientY },
            trigger: field.element,
        });
    };
    /** Route persistence failures through existing non-blocking SugarCubes feedback. */
    #run = (operation) => {
        void operation.catch(this.#options.reportError);
    };
}
