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
import {
  createNumericSlider,
  createBooleanCheckbox,
  createBooleanSegments,
} from './ControlFactory.js';
import type { NativeSeedReuseResolver } from './ComfyNativeSeedControl.js';

/** Retain owned widget controls while the native graph's independent presentation is suspended. */
export class NativeFieldAppearance {
  readonly #adapter: NativeCardAdapter;
  readonly #titlebars = new NativeCardTitlebar();
  readonly #labels = new NativeLabelPresentation();
  readonly #toggleRows = new NativeToggleRows(createBooleanCheckbox);
  readonly #booleanSegments = new NativeBooleanSegments(createBooleanSegments);
  readonly #advanced = new NativeAdvancedDisclosure();
  readonly #numeric = new NativeNumericControls(createNumericSlider);
  readonly #seeds = new NativeSeedControls();

  /** Bind one disjoint mount scope to the common component adapters and seed value owner. */
  constructor(
    app: unknown,
    private readonly kind: 'card' | 'widget',
    reuse?: NativeSeedReuseResolver,
  ) {
    this.#adapter = new NativeCardAdapter(app, reuse);
  }

  /** Index only native graph cards whose identified presentation is being refreshed. */
  beginBatch(document: Document, roots: readonly HTMLElement[]): void {
    if (this.kind === 'card' && roots.length) this.#adapter.beginBatch(document, roots);
  }

  /** Release transient indexes after each presentation batch. */
  endBatch(): void {
    this.#adapter.endBatch();
  }

  /** Reconcile the same numeric, seed, boolean and disclosure owners for either mount kind. */
  present(root: HTMLElement, themed = true): void {
    const card =
      this.kind === 'widget' ? this.#adapter.annotateWidget(root) : this.#adapter.annotate(root);
    if (!themed) {
      this.#numeric.reconcile(root, [
        ...new Set([
          ...card.seedFields.map((field) => field.numeric),
          ...card.numericFields.filter((field) => field.presentation === 'spinner-slider'),
        ]),
      ]);
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
  clear(): void {
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
