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
/** Apply one shared field treatment within an independently mounted renderer scope. */
import { NativeCardAdapter } from './NativeCardAdapter.js';
import { NativeCardTitlebar } from './NativeCardTitlebar.js';
import { NativeLabelPresentation } from './NativeLabelPresentation.js';
import { NativeToggleRows } from './NativeToggleRows.js';
import { NativeBooleanSegments } from './NativeBooleanSegments.js';
import { NativeAdvancedDisclosure } from './NativeAdvancedDisclosure.js';
import { NativeNumericControls } from './NativeNumericControls.js';
import { NativeSeedControls } from './NativeSeedControls.js';
import { createNumericSlider, createBooleanCheckbox, createBooleanSegments, } from './ControlFactory.js';
/** Retain owned widget controls while the native graph's independent presentation is suspended. */
export class NativeFieldAppearance {
    kind;
    #adapter;
    #titlebars = new NativeCardTitlebar();
    #labels = new NativeLabelPresentation();
    #toggleRows = new NativeToggleRows(createBooleanCheckbox);
    #booleanSegments = new NativeBooleanSegments(createBooleanSegments);
    #advanced = new NativeAdvancedDisclosure();
    #numeric = new NativeNumericControls(createNumericSlider);
    #seeds = new NativeSeedControls();
    /** Bind one disjoint mount scope to the common component adapters and seed value owner. */
    constructor(app, kind, reuse) {
        this.kind = kind;
        this.#adapter = new NativeCardAdapter(app, reuse);
    }
    /** Index only native graph cards whose identified presentation is being refreshed. */
    beginBatch(document, roots) {
        if (this.kind === 'card' && roots.length)
            this.#adapter.beginBatch(document, roots);
    }
    /** Release transient indexes after each presentation batch. */
    endBatch() {
        this.#adapter.endBatch();
    }
    /** Reconcile the same numeric, seed, boolean and disclosure owners for either mount kind. */
    present(root, themed = true) {
        const card = this.kind === 'widget' ? this.#adapter.annotateWidget(root) : this.#adapter.annotate(root);
        if (!themed) {
            this.#numeric.reconcile(root, card.seedFields.map((field) => field.numeric));
            this.#seeds.reconcile(root, card.seedFields);
            return;
        }
        this.#labels.reconcile(root, card.labelElements);
        if (this.kind === 'card')
            this.#titlebars.reconcile(root, card.header, card.collapsed, card.canCollapse);
        this.#toggleRows.reconcile(root);
        this.#booleanSegments.reconcile(root);
        this.#advanced.reconcile(card);
        this.#numeric.reconcile(root, card.numericFields);
        this.#seeds.reconcile(root, card.seedFields);
    }
    /** Release only this renderer scope, preserving controls mounted by the other scope. */
    clear() {
        this.#adapter.clear();
        this.#titlebars.clear();
        this.#labels.clear();
        this.#toggleRows.clear();
        this.#booleanSegments.clear();
        this.#advanced.clear();
        this.#numeric.clear();
        this.#seeds.clear();
    }
}
