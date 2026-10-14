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
/** Own one settings draft and its bounded live example within the SugarCubes modal lifecycle. */
import { ModalShell, type ModalAdapter } from '../dialogs/ModalShell.js';
import type { OutputSavingClient } from './OutputSavingClient.js';
import { OutputSavingForm } from './OutputSavingForm.js';
import type { OutputSavingSnapshot } from './OutputSavingPreferences.js';

/** Keep cancellation, preview responses and committed edits fenced to the current dialog. */
export class OutputSavingDialog {
  readonly #shell: ModalShell;
  #epoch = 0;

  /** Receive transport and modal adapters independently of host graph or canvas state. */
  constructor(
    private readonly document: Document,
    private readonly client: OutputSavingClient,
    adapter: ModalAdapter,
  ) {
    this.#shell = new ModalShell({ adapter, dialogClassName: 'sugarcubes-output-saving-dialog' });
  }

  /** Edit a server snapshot without persisting drafts or resetting workflow-local source choices. */
  async open(snapshot: OutputSavingSnapshot): Promise<OutputSavingSnapshot | null> {
    const epoch = ++this.#epoch;
    const form = new OutputSavingForm(this.document, snapshot);
    let timer: ReturnType<typeof setTimeout> | null = null;
    let preview: AbortController | null = null;
    let busy = false;
    let accepted: OutputSavingSnapshot | null = null;
    /** Cancel queued and in-flight examples before a new draft or save begins. */
    const cancelPreview = () => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      preview?.abort();
      preview = null;
    };
    /** Render only the latest live draft, retaining useful errors without flashing loading text. */
    const refreshExample = () => {
      cancelPreview();
      if (busy) return;
      timer = setTimeout(() => {
        timer = null;
        const controller = new AbortController();
        preview = controller;
        let draft;
        try {
          draft = form.read();
        } catch (error: unknown) {
          this.#shell.setError(
            error instanceof Error ? error.message : 'The output settings are invalid.',
          );
          return;
        }
        void this.client
          .preview(draft, controller.signal)
          .then((result) => {
            if (epoch !== this.#epoch || controller.signal.aborted) return;
            form.example.textContent = result.examplePath;
            this.#shell.setError('');
          })
          .catch((error: unknown) => {
            if (epoch !== this.#epoch || controller.signal.aborted) return;
            this.#shell.setError(
              error instanceof Error ? error.message : 'The output example is unavailable.',
            );
          });
      }, 250);
    };
    form.root.addEventListener('input', refreshExample);
    form.root.addEventListener('change', refreshExample);
    /** Leave failed edits visible and allow retry while blocking competing transactions. */
    const save = async () => {
      if (busy) return;
      cancelPreview();
      busy = true;
      form.setBusy(true);
      this.#shell.setBusy(true);
      try {
        const result = await this.client.save(form.read());
        if (epoch !== this.#epoch) return;
        accepted = result;
        this.#shell.close(true);
      } catch (error: unknown) {
        this.#shell.setError(
          error instanceof Error ? error.message : 'Output settings could not be saved.',
        );
      } finally {
        busy = false;
        form.setBusy(false);
        this.#shell.setBusy(false);
      }
    };
    try {
      await this.#shell.open({
        title: 'Output files',
        description: [
          'Cube image outputs are saved using these settings. Each workflow can mute saving for individual cubes.',
          'Workflow management applies these settings to supported save nodes and optionally completed preview nodes. Live sampler previews remain temporary.',
        ],
        body: form.root,
        confirmLabel: 'Save settings',
        cancelResult: null,
        initialFocus: () => form.folder,
        onConfirm: () => {
          void save();
        },
      });
      return accepted;
    } finally {
      if (epoch === this.#epoch) this.#epoch++;
      cancelPreview();
      form.root.removeEventListener('input', refreshExample);
      form.root.removeEventListener('change', refreshExample);
    }
  }
}
