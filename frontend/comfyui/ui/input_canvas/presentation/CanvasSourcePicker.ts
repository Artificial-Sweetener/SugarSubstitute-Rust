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
/** Present source intent through the existing native field renderer. */
import type {
  CanvasInputSource,
  CanvasInputTarget,
  CanvasSourceContext,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import { CanvasSourceUnavailable } from '../domain/CanvasSourceAvailability.js';
import { CANVAS_INPUT_CHOICES, chooseCanvasInput } from '../domain/CanvasInputSource.js';
import type {
  NativeNodeFields,
  NativeNodeChoiceMount,
  NativeNodeActionMount,
} from '../../surface/NativeNodeFields.js';

/** Keep capability discovery, persistence and file transport in their owning collaborators. */
export interface CanvasSourcePickerPort {
  readonly canvasId: string;
  readonly disabled: boolean;
  source(): CanvasInputSource;
  context(): CanvasSourceContext | null;
  refresh(): Promise<void>;
  change(source: CanvasInputSource): void | Promise<void>;
  upload(file: File): Promise<string>;
  report(error: unknown): void;
}
/** Supply source values to native fields without owning their label, combo or popup rendering. */
export class CanvasSourcePicker {
  readonly element: HTMLElement;
  readonly #kind: NativeNodeChoiceMount;
  readonly #layer: NativeNodeChoiceMount;
  readonly #file: HTMLInputElement;
  readonly #fileField: NativeNodeActionMount;
  #busy = false;
  #alive = true;
  /** Share Comfy's field renderer and the existing command-button owner. */
  constructor(
    document: Document,
    private readonly port: CanvasSourcePickerPort,
    fields: NativeNodeFields,
    fieldRoot: HTMLElement,
    fieldLabel: string | null,
  ) {
    this.element = document.createElement('div');
    this.element.className = 'sc-canvas-source';
    this.#kind = fields.mountChoice(
      fieldRoot,
      fieldLabel ? fieldLabel + ' Source' : 'Source',
      this.#choose,
      'Source',
      this.#prepare,
    );
    this.#layer = fields.mountChoice(
      fieldRoot,
      fieldLabel ? fieldLabel + ' Layer' : 'Layer',
      this.#chooseLayer,
      'Layer',
      this.#prepare,
    );
    this.#file = document.createElement('input');
    this.#file.type = 'file';
    this.#file.accept = 'image/*';
    this.#file.hidden = true;
    this.#file.setAttribute('aria-label', 'Source file');
    this.#fileField = fields.mountActionField(
      fieldRoot,
      fieldLabel ? fieldLabel + ' File' : 'File',
      'Browse…',
      () => {
        if (!this.#busy && !this.port.disabled) this.#file.click();
      },
      'File',
    );
    this.element.append(this.#file);
    this.#file.addEventListener('change', this.#upload);
    this.refresh();
  }
  /** Reconcile capability restrictions and exact saved identities through the mounted native fields. */
  refresh(): void {
    const source = this.port.source(),
      context = this.port.context();
    const kind = source.kind === 'file' ? 'file' : source.target.kind;
    this.#kind.update(
      kind,
      CANVAS_INPUT_CHOICES.map((choice) => ({
        value: choice.kind,
        label: choice.label,
        blockers:
          choice.kind === 'file' || context?.targets.includes(choice.kind)
            ? []
            : ['the connected canvas'],
      })),
      this.#busy || this.port.disabled,
    );
    const explicit =
      source.kind === 'canvas' &&
      (source.target.kind === 'layer' || source.target.kind === 'composition-from-layer');
    this.#file.disabled = this.#busy || this.port.disabled;
    const file = source.kind === 'file' ? source.file : null;
    this.#fileField.update(
      file?.split('/').at(-1) ?? 'No file selected',
      this.#busy || this.port.disabled,
      source.kind === 'file',
      file ?? '',
    );
    if (
      !explicit ||
      (source.target.kind !== 'layer' && source.target.kind !== 'composition-from-layer')
    ) {
      this.#layer.update('', [], this.#busy || this.port.disabled, false);
      return;
    }
    const layers = context?.document?.id === source.documentId ? context.layers : [];
    const target = source.target;
    const value = layers.some((layer) => layer.id === target.layerId) ? target.layerId : '';
    this.#layer.update(
      value,
      [
        { value: '', label: 'Choose a layer', blockers: ['a document layer'] },
        ...layers.map((layer) => ({
          value: layer.id,
          label: ' '.repeat(layer.depth) + layer.name,
          blockers: [],
        })),
      ],
      this.#busy || this.port.disabled || !layers.length,
    );
  }
  /** Retire native mounts before removing the owning source surface. */
  dispose(): void {
    this.#alive = false;
    this.#fileField.dispose();
    this.#kind.dispose();
    this.#layer.dispose();
    this.element.remove();
  }
  /** Discover current choices and preview pixels as part of using the source picker. */
  readonly #prepare = async (): Promise<boolean> => {
    if (this.#busy || this.port.disabled || !this.#alive) return false;
    try {
      await this.port.refresh();
      if (!this.#alive) return false;
      this.refresh();
      return true;
    } catch (error: unknown) {
      if (this.#alive) this.port.report(error);
      return false;
    }
  };
  /** Resolve the initial explicit layer only when the user chooses a layer-based target. */
  readonly #choose = (value: string): void => {
    const kind = CANVAS_INPUT_CHOICES.find((choice) => choice.kind === value)?.kind;
    if (!kind) return;
    const context = this.port.context();
    let target: CanvasInputTarget;
    if (kind === 'file') {
      const current = this.port.source();
      target = { kind, file: current.kind === 'file' ? current.file : null };
    } else if (kind === 'layer' || kind === 'composition-from-layer') {
      const layerId = context?.currentLayerId;
      if (!layerId) {
        this.port.report(new CanvasSourceUnavailable('Select a layer'));
        this.refresh();
        return;
      }
      target = { kind, layerId };
    } else target = { kind };
    this.#change(target);
  };
  /** Preserve the selected operation when the user chooses another exact document layer. */
  readonly #chooseLayer = (layerId: string): void => {
    const source = this.port.source();
    if (
      source.kind === 'canvas' &&
      (source.target.kind === 'layer' || source.target.kind === 'composition-from-layer')
    )
      this.#change({ kind: source.target.kind, layerId });
  };
  /** Persist validated user intent through the consumer's authoritative state transition. */
  #change(target: CanvasInputTarget): void {
    void this.#operation(async () => {
      await this.port.change(chooseCanvasInput(this.port.canvasId, this.port.context(), target));
    });
  }
  /** Upload once and ignore the result if its owning surface has retired. */
  readonly #upload = (): void => {
    const file = this.#file.files?.[0];
    if (!file) return;
    void this.#operation(async () => {
      const path = await this.port.upload(file);
      if (this.#alive) await this.port.change({ kind: 'file', file: path });
      this.#file.value = '';
    });
  };
  /** Keep native controls disabled until one source intent settles. */
  async #operation(action: () => Promise<void>): Promise<void> {
    if (this.#busy || this.port.disabled || !this.#alive) return;
    this.#busy = true;
    this.refresh();
    try {
      await action();
    } catch (error: unknown) {
      if (this.#alive) this.port.report(error);
    } finally {
      this.#busy = false;
      if (this.#alive) this.refresh();
    }
  }
}
