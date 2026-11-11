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
/** One literal, source-backed LoRA bar represents every host resolution state. */
import type { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import type { LoraResource } from '../../services/src/index.js';
import type { EditorView } from '@codemirror/view';
import { mountWeightInput, releaseWeightInput } from './weight-input-mount.js';
import { weightValue } from './weight-value.js';
import { SourceWidget, type SourceToken } from './projection-widgets.js';
import {
  pendingLoraMedia,
  unavailableLoraMedia,
  type PreparedLoraMedia,
} from './lora-media-contracts.js';

/** Compact donor labels use Unicode code points; exact names and metadata remain available through the accessible label. */
function label(text: string, maximum: number): string {
  const characters = Array.from(text.trim());
  return characters.length <= maximum
    ? characters.join('')
    : characters.slice(0, maximum - 3).join('') + '...';
}

/** Fall back to the literal source basename when the host has no authoritative display title. */
function basename(name: string): string {
  return name.replaceAll('\\', '/').split('/').at(-1) || name;
}

/** Prepare human-readable resolution feedback without treating missing authority as model absence. */
function feedback(resource: LoraResource): string {
  switch (resource.resolution) {
    case 'found':
      return 'Installed';
    case 'missing':
      return 'Missing model';
    case 'ambiguous':
      return resource.candidateCount === null
        ? 'Ambiguous model name'
        : `Ambiguous model name (${resource.candidateCount} candidates)`;
    case 'pending-authority':
      return 'Waiting for catalog authority';
    case 'unavailable':
      return 'Catalog unavailable';
  }
}

/** Native atomic rendering keeps one layout and source identity independently of resolution. */
export class LoraWidget extends SourceWidget {
  readonly resource: LoraResource;
  readonly weight: string;
  readonly title: string;
  readonly subtitle: string;
  readonly status: string;
  readonly titleLabel: string;
  /** Compact version text retains secondary paint; the primary separator has its own literal span. */
  readonly subtitleLabel: string;
  readonly initial: string;
  readonly media: PreparedLoraMedia['status'];
  readonly preparedMedia: PreparedLoraMedia;
  readonly adjustment: 'none' | 'wheel' | 'pointer';
  readonly draftKey: symbol | null;
  /** Capture fully prepared display inputs; DOM construction performs no parsing, catalog traversal or media acquisition. */
  constructor(
    token: SourceToken,
    resource: LoraResource,
    weight: string,
    adjustment: 'none' | 'wheel' | 'pointer' = 'none',
    media?: PreparedLoraMedia,
    draftKey: symbol | null = null,
  ) {
    super(token);
    this.resource = resource;
    this.weight = weight;
    this.adjustment = adjustment;
    this.draftKey = draftKey;
    this.title =
      resource.resolution === 'found'
        ? resource.title || resource.model.basename
        : basename(resource.name);
    this.subtitle = resource.resolution === 'found' ? (resource.subtitle?.trim() ?? '') : '';
    this.status = feedback(resource);
    this.titleLabel = label(this.title, 20);
    this.subtitleLabel = this.subtitle ? label(this.subtitle, 15) : '';
    this.initial = Array.from(this.title.trim())[0]?.toUpperCase() ?? 'L';
    this.preparedMedia =
      media ??
      (resource.resolution === 'found' && resource.thumbnail.status === 'ready'
        ? pendingLoraMedia
        : unavailableLoraMedia);
    this.media = this.preparedMedia.status;
  }
  /** Equivalent immutable descriptors retain actual native DOM across preparation and selection. */
  override eq(other: LoraWidget): boolean {
    return this.sameContent(other) && this.adjustment === other.adjustment;
  }
  /** Native feedback changes only its two owned class tokens; equivalent content retains decoded media and numeric subtree identity. */
  override updateDOM(dom: HTMLElement, _view: EditorView, from: LoraWidget): boolean {
    if (!this.sameContent(from)) return false;
    dom.classList.toggle('sp-wheel-ready', this.adjustment === 'wheel');
    dom.classList.toggle('sp-adjustment-content', this.adjustment === 'pointer');
    return true;
  }
  /** One immutable comparison owns every constructor operand that can change the retained child graph or attributes. */
  private sameContent(other: LoraWidget): boolean {
    return (
      this.resource.name === other.resource.name &&
      this.resource.resolution === other.resource.resolution &&
      this.title === other.title &&
      this.subtitle === other.subtitle &&
      this.status === other.status &&
      this.media === other.media &&
      this.preparedMedia === other.preparedMedia &&
      this.weight === other.weight &&
      this.draftKey === other.draftKey
    );
  }
  /** Create the shared bar through safe literal DOM APIs; CodeMirror owns pointer selection and lifecycle. */
  override toDOM(view: EditorView): HTMLElement {
    return this.createDOM(view, null);
  }
  /** Preserve the same canonical chrome while the readonly owner admits viewport raster pixels separately. */
  override toReadonlyDOM(view: EditorView, media: ReadonlyWidgetMedia): HTMLElement {
    return this.createDOM(view, media);
  }
  /** One native constructor owns both normal rendering and readonly deferred absolute media. */
  private createDOM(view: EditorView, media: ReadonlyWidgetMedia | null): HTMLElement {
    const chip = document.createElement('span');
    chip.className =
      'sp-lora-chip' +
      (this.adjustment === 'wheel'
        ? ' sp-wheel-ready'
        : this.adjustment === 'pointer'
          ? ' sp-adjustment-content'
          : '');
    chip.dataset.resolution = this.resource.resolution;
    chip.dataset.media = this.media;
    chip.setAttribute('role', this.draftKey ? 'group' : 'img');
    chip.setAttribute(
      'aria-label',
      `LoRA ${this.resource.name}, ${this.title}${this.subtitle ? ', ' + this.subtitle : ''}, ${this.status}, weight ${this.weight}`,
    );
    chip.title = `${this.title}${this.subtitle ? ' - ' + this.subtitle : ''} · ${this.status}`;
    const backing = document.createElement('span');
    backing.className = 'sp-lora-backing';
    backing.setAttribute('aria-hidden', 'true');
    if (this.preparedMedia.status === 'ready' && media) {
      media.defer(backing, this.preparedMedia.raster);
    } else if (this.preparedMedia.status === 'ready') {
      const raster = this.preparedMedia.raster;
      const canvas = document.createElement('canvas');
      canvas.className = 'sp-lora-media';
      canvas.width = raster.width;
      canvas.height = raster.height;
      canvas.setAttribute('aria-hidden', 'true');
      this.preparedMedia.raster.paint(canvas);
      backing.append(canvas);
    }
    const initial = document.createElement('span');
    initial.className = 'sp-lora-initial';
    initial.textContent = this.initial;
    initial.setAttribute('aria-hidden', 'true');
    const title = document.createElement('span');
    title.className = 'sp-lora-title';
    title.textContent = this.titleLabel;
    const subtitle = document.createElement('span');
    subtitle.className = 'sp-lora-subtitle';
    subtitle.textContent = this.subtitleLabel;
    const weight = document.createElement('span');
    weight.className = 'sp-lora-weight';
    if (this.draftKey) mountWeightInput(view, this.draftKey, weight, this.weight);
    else weight.append(weightValue(this.weight));
    chip.append(backing, initial, title);
    if (this.subtitleLabel) {
      const separator = document.createElement('span');
      separator.className = 'sp-lora-separator';
      separator.textContent = ' - ';
      chip.append(separator);
    }
    chip.append(subtitle, weight);
    return chip;
  }
  /** CodeMirror widget removal releases the input mount while preserving native parent ownership. */
  override destroy(dom: HTMLElement): void {
    const cell = dom.querySelector<HTMLElement>('.sp-lora-weight');
    if (cell) releaseWeightInput(cell);
  }
}
