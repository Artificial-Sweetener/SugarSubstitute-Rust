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
/** Lease replaced source controls and native media without changing loader values or serialization. */
import type { ComfyNode } from '../../types/graph.js';
import { NativeWidgetPresentation } from '../../surface/NativeWidgetPresentation.js';
import { NativeNodeMediaReplacement } from '../../surface/NativeNodeMediaPresentation.js';

interface WidgetOwner {
  readonly lease: NativeWidgetPresentation;
  references: number;
}
const widgets = new WeakMap<ComfyNode, WidgetOwner>();

/** Share one widget visibility owner across simultaneous projections of the same exact loader. */
export class InputLoaderPresentation {
  readonly #widgets: WidgetOwner;
  readonly #media: NativeNodeMediaReplacement;
  #disposed = false;
  /** Replace filename/upload presentation while retaining channel and all other loader settings. */
  constructor(
    private readonly node: ComfyNode,
    root: HTMLElement,
    private readonly fields: readonly string[],
  ) {
    let owner = widgets.get(node);
    if (!owner) {
      owner = { lease: new NativeWidgetPresentation('replacement'), references: 0 };
      widgets.set(node, owner);
    }
    owner.references++;
    this.#widgets = owner;
    this.#media = new NativeNodeMediaReplacement(root);
    this.refresh();
  }
  /** Reconcile native widgets introduced by host renderer or uploader lifecycles. */
  refresh(): boolean {
    return this.#widgets.lease.reconcile(
      new Set(
        (this.node.widgets ?? []).filter(
          (widget) =>
            this.fields.includes(widget.name ?? '') ||
            widget.name === 'upload' ||
            widget.type === 'image_upload' ||
            (['SimpleSyrup.LoadMaskBatch', 'SimpleSyrup.LoadImageList'].includes(
              this.node.type ?? this.node.class_type ?? '',
            ) &&
              (widget.name === 'simple_syrup_replace_media' ||
                widget.name === 'simple_syrup_add_media')),
        ),
      ),
    );
  }
  /** Restore graph-owned controls after the last replacement projection retires. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#media.dispose();
    this.#widgets.references--;
    if (!this.#widgets.references) {
      this.#widgets.lease.dispose();
      widgets.delete(this.node);
    }
  }
}
