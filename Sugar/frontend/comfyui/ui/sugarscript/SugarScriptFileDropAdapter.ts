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
/** Accept `.sugar` files through Comfy's canvas drop surface without an editor UI. */

const MAX_SOURCE_BYTES = 1_000_000;

interface SugarScriptFileDropFeedback {
  push?(severity: string, summary: string, detail?: string): unknown;
}

export interface SugarScriptFileDropAdapterOptions {
  document: Document;
  importSource(source: string): Promise<unknown>;
  feedback?: SugarScriptFileDropFeedback | null;
  readErrorMessage(error: unknown): string;
}

/** Own capture-phase interception only for a single bounded `.sugar` file. */
export class SugarScriptFileDropAdapter {
  readonly #document: Document;
  readonly #importSource: (source: string) => Promise<unknown>;
  readonly #feedback: SugarScriptFileDropFeedback | null;
  readonly #readErrorMessage: (error: unknown) => string;
  readonly #onDragOver = (event: DragEvent): void => this.#handleDragOver(event);
  readonly #onDrop = (event: DragEvent): void => this.#handleDrop(event);
  #active = false;

  /** Bind document, import use case, and host feedback. */
  constructor(options: SugarScriptFileDropAdapterOptions) {
    this.#document = options.document;
    this.#importSource = options.importSource;
    this.#feedback = options.feedback ?? null;
    this.#readErrorMessage = options.readErrorMessage;
  }

  /** Start idempotent file interception without changing other Comfy drops. */
  setup(): void {
    if (this.#active) return;
    this.#active = true;
    this.#document.addEventListener('dragover', this.#onDragOver, true);
    this.#document.addEventListener('drop', this.#onDrop, true);
  }

  /** Release document listeners when the host lifecycle ends. */
  dispose(): void {
    if (!this.#active) return;
    this.#active = false;
    this.#document.removeEventListener('dragover', this.#onDragOver, true);
    this.#document.removeEventListener('drop', this.#onDrop, true);
  }

  /** Advertise copy behavior only when the payload is one `.sugar` file. */
  #handleDragOver(event: DragEvent): void {
    const file = readSugarFile(event.dataTransfer);
    if (!file) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
  }

  /** Read and import one bounded source file while suppressing Comfy's unknown-file path. */
  #handleDrop(event: DragEvent): void {
    const file = readSugarFile(event.dataTransfer);
    if (!file) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    void this.#importFile(file);
  }

  /** Contain file read and compilation failures at the user-visible host boundary. */
  async #importFile(file: File): Promise<void> {
    try {
      if (file.size > MAX_SOURCE_BYTES) throw new Error('SugarScript source exceeds 1 MB.');
      const source = await file.text();
      await this.#importSource(source);
      this.#feedback?.push?.('success', 'SugarScript workflow imported', file.name);
    } catch (error: unknown) {
      this.#feedback?.push?.('error', 'SugarScript import failed', this.#readErrorMessage(error));
    }
  }
}

/** Return exactly one `.sugar` file and leave every other drop untouched. */
function readSugarFile(transfer: DataTransfer | null): File | null {
  const files = transfer ? [...transfer.files] : [];
  if (files.length !== 1) return null;
  const file = files[0];
  return file?.name.toLocaleLowerCase().endsWith('.sugar') ? file : null;
}
