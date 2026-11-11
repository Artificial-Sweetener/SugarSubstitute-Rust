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
/** Coordinate an explicit source-settings edit using the active adapter-owned workflow port. */
import type { OutputSavingConfiguration } from './OutputSavingConfiguration.js';
import type { SourceSavingEditorPort } from './SourceSavingEditorPort.js';
import type { SourceSavingDialog } from './SourceSavingDialog.js';

/** Keep source discovery and history outside settings presentation. */
export class SourceSavingControls {
  #port: SourceSavingEditorPort | null = null;
  #pending: Promise<void> | null = null;
  /** Bind committed settings and the shared source modal. */
  constructor(
    private readonly configuration: OutputSavingConfiguration,
    private readonly dialog: SourceSavingDialog,
  ) {}

  /** Lease the active source provider without allowing teardown to clear a replacement binding. */
  bind(port: SourceSavingEditorPort): () => void {
    this.#port = port;
    return () => {
      if (this.#port === port) this.#port = null;
    };
  }

  /** Open one deliberate edit while revalidating shared settings before capture. */
  open(): Promise<void> {
    if (this.#pending) return this.#pending;
    const port = this.#port;
    if (!port) return Promise.reject(new Error('The workflow output controls are unavailable.'));
    let session;
    try {
      session = port.begin();
    } catch (error: unknown) {
      return Promise.reject(
        error instanceof Error
          ? error
          : new Error('Output source settings could not be read.', { cause: error }),
      );
    }
    this.#pending = this.configuration
      .load()
      .then((snapshot) => {
        if (this.#port !== port || !session.current())
          throw new Error('The workflow changed before output settings opened.');
        return this.dialog.open(session, snapshot.preferences);
      })
      .finally(() => {
        this.#pending = null;
      });
    return this.#pending;
  }
}
