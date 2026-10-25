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
/** Reconcile persistent loader previews while the content owner coordinates sources and interpretation. */
import type { InputEditorBinding, InputImageAssociation } from '../application/InputEditorPorts.js';
import type { CanvasSourcePreviews } from '../application/CanvasSourcePreviews.js';
import { InputPreviewContent } from '../application/InputPreviewContent.js';
import type { InputEntry } from '../domain/InputBinding.js';
import { InputPreviewSurface } from './InputPreviewSurface.js';
import { InputPreviewResolutionObserver } from './InputPreviewResolutionObserver.js';
import { readPluginMode } from '../../core/PluginMode.js';
interface Row {
  readonly resolution: InputPreviewResolutionObserver;
  readonly surface: InputPreviewSurface;
  reference: string;
  content: InputPreviewContent | null;
}

/** Share one mounted preview lifecycle across value, target, settings and geometry refreshes. */
export class InputPreviewRows {
  readonly element: HTMLElement;
  readonly #empty: InputPreviewSurface;
  readonly #entries = new Map<string, Row>();
  #alive = true;
  /** Bind pixels and state through the existing source and interpretation owners. */
  constructor(
    document: Document,
    private readonly binding: InputEditorBinding,
    private readonly previews: CanvasSourcePreviews,
    private readonly report: (error: unknown) => void,
    private readonly association: InputImageAssociation | null,
  ) {
    this.element = document.createElement('div');
    this.element.className = 'sc-input-editor__rows';
    this.#empty = new InputPreviewSurface(document, true, () => {});
    this.#empty.element.dataset.inputEmpty = '';
    this.#empty.update('', true, false, true, () => {});
    if (binding.loader.kind === 'mask') this.#empty.black();
    else this.#empty.unavailable('Choose a source');
    this.element.append(this.#empty.element);
  }
  /** Reconcile only this loader's previews, retaining geometry and cached pixels during replacement. */
  refresh(): void {
    if (!this.#alive) return;
    const state = this.binding.state,
      ids = new Set(state.entries.map((entry) => entry.id));
    const passive = readPluginMode(this.element.ownerDocument);
    this.#empty.element.hidden = state.entries.length > 0;
    const canvas = this.binding.context?.document;
    if (canvas) this.#empty.frame(canvas.width, canvas.height);
    for (const [id, row] of this.#entries)
      if (!ids.has(id)) {
        row.content?.dispose();
        row.resolution.dispose();
        row.surface.element.remove();
        this.#entries.delete(id);
      }
    for (const [index, entry] of state.entries.entries()) {
      let row = this.#entries.get(entry.id);
      if (!row) {
        row = this.#row(entry);
        this.#entries.set(entry.id, row);
      }
      row.surface.update(entry.label, entry.id === state.selected, this.binding.busy, passive, () =>
        this.#select(entry.id),
      );
      const destination =
        this.element.querySelectorAll('[data-input-entry]')[index] ?? this.#empty.element;
      if (destination !== row.surface.element)
        this.element.insertBefore(row.surface.element, destination);
      const document = this.binding.context?.document;
      if (
        document &&
        this.binding.loader.kind === 'image' &&
        (entry.source.kind === 'canvas' || !entry.source.file)
      )
        row.surface.frame(document.width, document.height);
      if (this.binding.busy || this.binding.error) {
        row.content?.dispose();
        row.content = null;
        row.reference = '';
        if (this.binding.loader.kind === 'mask') row.surface.black();
        else if (this.binding.error) row.surface.unavailable('Source unavailable');
        else row.surface.loading();
        continue;
      }
      const size = row.resolution.resolution;
      if (!size) continue;
      const reference = InputPreviewContent.reference(this.binding, entry, size);
      if (row.reference !== reference) {
        const previous = row.content;
        row.reference = reference;
        row.surface.loading();
        row.content = this.#content(entry, row, size);
        previous?.dispose();
      } else row.content?.refresh();
    }
  }
  /** Explain a rejected source intent within its existing selected preview. */
  unavailable(message: string): void {
    const row = this.#entries.get(this.binding.state.selected ?? '');
    if (row?.content) row.content.unavailable(message);
    else if (this.binding.loader.kind === 'mask') (row?.surface ?? this.#empty).black();
    else (row?.surface ?? this.#empty).unavailable(message);
  }
  /** Release geometry and capture observations before accepting any late interpretation result. */
  dispose(): void {
    this.#alive = false;
    for (const row of this.#entries.values()) {
      row.content?.dispose();
      row.resolution.dispose();
    }
    this.#entries.clear();
    this.element.remove();
  }
  /** Create one literal, stable entry selection surface without native mask mutation intents. */
  #row(entry: InputEntry): Row {
    const surface = new InputPreviewSurface(
      this.element.ownerDocument,
      readPluginMode(this.element.ownerDocument),
      () => this.#select(entry.id),
    );
    surface.element.dataset.inputEntry = entry.id;
    const document = this.binding.context?.document;
    if (document) surface.frame(document.width, document.height);
    if (this.binding.loader.kind === 'mask') surface.black();
    return {
      surface,
      resolution: new InputPreviewResolutionObserver(surface.frameElement, () => this.refresh()),
      reference: '',
      content: null,
    };
  }
  /** Preserve ordinary browser entry selection without attaching actions to plugin previews. */
  #select(id: string): void {
    try {
      this.binding.select(id);
    } catch (error: unknown) {
      this.report(error);
    }
  }
  /** Keep interpretation and capture lifetimes outside the persistent preview's geometry owner. */
  #content(entry: InputEntry, row: Row, size: number): InputPreviewContent {
    return new InputPreviewContent(
      this.binding,
      this.previews,
      entry,
      size,
      (value) => {
        if (!this.#alive) return;
        if (value.kind === 'loading') row.surface.loading();
        else if (value.kind === 'black') row.surface.black();
        else if (value.kind === 'error') row.surface.unavailable(value.message);
        else {
          try {
            row.surface.paint(value.pixels, value.frame, value.background);
          } catch (error: unknown) {
            row.surface.unavailable('Preview unavailable');
            this.report(error);
          }
        }
      },
      this.report,
      this.association,
    );
  }
}
