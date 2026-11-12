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
/** Lease mask preparation controls through the ordinary themed node-field owner. */
import type {
  NativeNodeFields,
  NativeNodeNumberMount,
  NativeNodeChoiceMount,
} from '../../surface/NativeNodeFields.js';
import type { InputEditorBinding, InputImageAssociation } from '../application/InputEditorPorts.js';
import { DEFAULT_MASK_EDITS, type MaskEditSettings } from '../domain/MaskEditSettings.js';
import type { MaskPreviewMode } from '../domain/MaskPreviewGeometry.js';

/** Keep presentation-only preview framing distinct from persisted mask processing intent. */
export class MaskEditFields {
  readonly #numbers = new Map<keyof MaskEditSettings, NativeNodeNumberMount>();
  readonly #stopAssociation: () => void;
  #stopImage: () => void = () => {};
  #image: InputEditorBinding | null = null;
  #view: NativeNodeChoiceMount | null = null;
  #mode: MaskPreviewMode = 'context';
  #alive = true;
  /** Reuse binding persistence and graph-derived eligibility without constructing controls or discovering nodes. */
  constructor(
    private readonly binding: InputEditorBinding,
    private readonly association: InputImageAssociation | null,
    private readonly fields: NativeNodeFields,
    private readonly root: HTMLElement,
    private readonly showField: boolean,
    private readonly changeView: (mode: MaskPreviewMode) => void,
    private readonly report: (error: unknown) => void,
  ) {
    this.#stopAssociation = association?.subscribe(() => this.refresh()) ?? (() => {});
  }

  /** Expose editing only for the current scalar pair and connected canvas capability. */
  refresh(): void {
    if (!this.#alive) return;
    const image = this.association?.read() ?? null;
    if (this.#image !== image) {
      this.#stopImage();
      this.#image = image;
      this.#stopImage = image?.subscribe(() => this.refresh()) ?? (() => {});
    }
    if (this.binding.loader.kind !== 'mask' || !this.association?.editable()) {
      this.#release();
      return;
    }
    const settings = this.binding.state.edits ?? DEFAULT_MASK_EDITS;
    for (const [key, label] of [
      ['mask_dilation', 'Mask Dilation'],
      ['crop_factor', 'Crop Factor'],
      ['post_dilation', 'Post Dilation'],
      ['feather', 'Feather'],
    ] as const) {
      let mount = this.#numbers.get(key);
      if (!mount) {
        mount = this.fields.mountNumber(
          this.#label(label),
          (value) => this.#change(key, value),
          {
            min: key === 'mask_dilation' || key === 'post_dilation' ? -512 : 0,
            max: key === 'crop_factor' ? 100 : 512,
            step: key === 'crop_factor' ? 0.1 : 1,
            precision: key === 'crop_factor' ? 1 : 0,
            presentation: 'spinner-slider',
          },
          'mask-edit:' + key,
        );
        this.#numbers.set(key, mount);
      }
      mount.update(settings[key], this.binding.busy);
    }
    this.#view ??= this.fields.mountChoice(
      this.root,
      this.#label('Preview'),
      (value) => {
        if (value !== 'context' && value !== 'input') return;
        this.#mode = value;
        this.changeView(value);
        this.refresh();
      },
      'mask-edit:preview',
    );
    this.#view.update(
      this.#mode,
      [
        { value: 'context', label: 'Full context', blockers: [] },
        { value: 'input', label: 'Actual input', blockers: [] },
      ],
      this.binding.busy,
    );
  }

  /** Release observers and native presentation leases with their owning card. */
  dispose(): void {
    this.#alive = false;
    this.#stopAssociation();
    this.#stopImage();
    this.#release();
  }

  /** Admit each native numeric intent through shared settings validation and workflow history. */
  #change(key: keyof MaskEditSettings, value: number): void {
    try {
      if (!this.association?.editable()) return;
      const current = this.binding.state.edits ?? DEFAULT_MASK_EDITS;
      const normalized =
        key === 'crop_factor' && value > 0 && value < 1
          ? value > current.crop_factor
            ? 1
            : 0
          : value;
      this.binding.changeMaskEdits({ ...current, [key]: normalized });
    } catch (error: unknown) {
      this.report(error);
    } finally {
      this.refresh();
    }
  }

  /** Retain stable field identities in the native owner while unsupported pairs expose no editing fields. */
  #release(): void {
    for (const mount of this.#numbers.values()) mount.dispose();
    this.#numbers.clear();
    this.#view?.dispose();
    this.#view = null;
    if (this.#mode !== 'context') {
      this.#mode = 'context';
      this.changeView('context');
    }
  }

  /** Match the existing upload-field labels when a node exposes multiple independent loaders. */
  #label(label: string): string {
    return this.showField ? this.binding.loader.field + ' ' + label : label;
  }
}
