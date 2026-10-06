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
/** Retain one field's version interaction and transfer state across native option remounts. */
import type { ModelSelectionReceiver } from './ModelPickerBinding.js';
import type { ModelCatalogItem } from './ModelCatalog.js';
import type { ModelPickerActions, ModelPickerFeedback } from './ModelPickerActions.js';
import type {
  ModelSelectionTarget,
  ModelTransferOperation,
  ModelTransferModel,
  ModelTransferState,
} from './ModelTransfer.js';
import { ModelVersionDialog } from './ModelVersionDialog.js';

/** Own field-scoped transfer feedback while resolving each commit against current host choices. */
export class ModelPickerSession implements ModelSelectionTarget {
  readonly #document: Document;
  readonly #resolve: () => ModelSelectionReceiver | null;
  readonly #listeners = new Set<() => void>();
  #progress: ModelTransferState | null = null;
  #dialog: ModelVersionDialog | null = null;
  #active = true;
  #transfer = 0;
  #controller: AbortController | null = null;

  /** Bind a receiver that survives presentation replacement but rejects removed workflow fields. */
  constructor(document: Document, resolve: () => ModelSelectionReceiver | null) {
    this.#document = document;
    this.#resolve = resolve;
  }
  /** Read the current workflow choice independently of the inspected model family. */
  current(): string {
    return this.#resolve()?.readValue() ?? '';
  }
  /** Expose the current transfer without allowing views to own its state. */
  read(): ModelTransferState | null {
    return this.#progress;
  }
  /** Follow progress until the temporary field renderer is replaced or removed. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Write only an exact legal host option, preserving host path separator conventions. */
  select(filename: string): void {
    const binding = this.#active ? this.#resolve() : null;
    const legal = binding?.options.find(
      (option) => option.replaceAll('\\', '/') === filename.replaceAll('\\', '/'),
    );
    if (!binding || legal === undefined || binding.disabled())
      throw new Error('The model is installed, but its current Comfy field is unavailable.');
    binding.writeValue(legal);
  }
  /** Publish one scoped transfer and clear it on success, cancellation, or failure. */
  async download(
    operation: ModelTransferOperation,
    model: ModelTransferModel,
    signal?: AbortSignal,
  ): Promise<string> {
    if (!this.#active || this.#progress)
      throw new Error('The model field is unavailable or already downloading.');
    const transfer = ++this.#transfer;
    const controller = new AbortController();
    this.#controller = controller;
    const cancel = (): void => controller.abort();
    signal?.addEventListener('abort', cancel, { once: true });
    if (signal?.aborted) cancel();
    this.#publish({ message: 'Waiting to download…', percentage: null, model });
    try {
      if (controller.signal.aborted) throw new DOMException('Cancelled', 'AbortError');
      const filename = await operation((progress) => {
        if (this.#active && !controller.signal.aborted && transfer === this.#transfer)
          this.#publish({ ...progress, model });
      }, controller.signal);
      if (!this.#active || controller.signal.aborted)
        throw new DOMException('Cancelled', 'AbortError');
      return filename;
    } finally {
      signal?.removeEventListener('abort', cancel);
      if (this.#controller === controller) this.#controller = null;
      if (transfer === this.#transfer) {
        this.#transfer += 1;
        this.#publish(null);
      }
    }
  }
  /** Keep an accepted acquisition alive until its refreshed legal choice is committed or its field disappears. */
  async useDownload(operation: ModelTransferOperation, model: ModelTransferModel): Promise<void> {
    try {
      this.select(await this.download(operation, model));
    } catch (error: unknown) {
      if (this.#active) throw error;
    }
  }
  /** Browse versions without tying the modal to a temporary combobox renderer. */
  async versions(
    item: ModelCatalogItem,
    actions: ModelPickerActions,
    feedback: ModelPickerFeedback,
  ): Promise<void> {
    if (!this.#active || this.#progress) return;
    this.#dialog?.close();
    const dialog = new ModelVersionDialog(this.#document, item, actions, this, feedback);
    this.#dialog = dialog;
    try {
      await dialog.open();
    } finally {
      if (this.#dialog === dialog) this.#dialog = null;
    }
  }
  /** Close interactions when the actual node or supported renderer disappears. */
  dispose(): void {
    this.#active = false;
    this.#controller?.abort();
    this.#controller = null;
    this.#dialog?.close();
    this.#dialog = null;
    this.#transfer += 1;
    this.#publish(null);
    this.#listeners.clear();
  }
  /** Notify subscribers through the single progress state owner. */
  #publish(progress: ModelTransferState | null): void {
    this.#progress = progress;
    for (const listener of this.#listeners) listener();
  }
}
