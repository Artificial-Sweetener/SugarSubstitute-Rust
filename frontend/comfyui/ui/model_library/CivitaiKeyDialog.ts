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
/** Own explicit secure key entry through the shared Fluent modal and credential port. */
import { ModalShell } from '../dialogs/ModalShell.js';
import type { CivitaiCredentialPort } from './ModelDiscovery.js';

/** Keep password material local to a short-lived purpose dialog and server-owned secure storage. */
export class CivitaiKeyDialog {
  readonly #shell: ModalShell;
  readonly #document: Document;
  readonly #credentials: CivitaiCredentialPort;
  #controller: AbortController | null = null;

  /** Bind the existing credential owner without storing keys in preferences or the model catalog. */
  constructor(document: Document, credentials: CivitaiCredentialPort) {
    this.#document = document;
    this.#credentials = credentials;
    this.#shell = new ModalShell({
      adapter: { getDocument: () => document, getWindow: () => document.defaultView },
      variantClassName: 'sc-model-key-overlay',
      dialogClassName: 'sc-model-key-dialog',
    });
  }

  /** Save only deliberate user input and return whether secure configuration succeeded. */
  async open(): Promise<boolean> {
    const body = this.#document.createElement('div');
    const explanation = this.#document.createElement('p');
    explanation.className = 'sc-model-key-explanation';
    explanation.textContent =
      'This model requires a CivitAI API key to download. Create a key in your CivitAI account settings and paste it below to continue.';
    const label = this.#document.createElement('label');
    label.className = 'sugarcubes-modal__field';
    label.textContent = 'CivitAI API key';
    const input = this.#document.createElement('input');
    input.type = 'password';
    input.className = 'p-inputtext p-component sugarcubes-modal__text-input';
    input.autocomplete = 'off';
    input.spellcheck = false;
    input.setAttribute('aria-label', 'CivitAI API key');
    const link = this.#document.createElement('a');
    link.href = 'https://civitai.com/user/account';
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.className = 'sc-model-key-settings';
    link.textContent = 'Open CivitAI account settings';
    const feedback = this.#document.createElement('div');
    feedback.setAttribute('role', 'alert');
    feedback.style.minHeight = '40px';
    label.append(input);
    body.append(explanation, label, link, feedback);
    let busy = false;
    const closed = this.#shell.open({
      title: 'Add CivitAI API key',
      body,
      confirmLabel: 'Save key',
      cancelLabel: 'Cancel',
      allowOverlayClose: false,
      onConfirm: () => {
        if (busy || !input.value.trim()) return;
        busy = true;
        const controller = new AbortController();
        this.#controller = controller;
        this.#shell.setBusy(true);
        feedback.textContent = '';
        const key = input.value.trim();
        input.value = '';
        void this.#credentials
          .save(key, controller.signal)
          .then((status) => {
            if (controller.signal.aborted) return;
            if (!status.configured) throw new Error('The CivitAI key could not be saved.');
            this.#shell.close(true);
          })
          .catch((error: unknown) => {
            if (controller.signal.aborted) return;
            feedback.textContent =
              error instanceof Error ? error.message : 'The CivitAI key could not be saved.';
            console.error('SugarCubes secure credential command failed.', error);
          })
          .finally(() => {
            busy = false;
            this.#shell.setBusy(false);
            this.#shell.setConfirmEnabled(!!input.value.trim());
          });
      },
    });
    this.#shell.elements.dialog?.setAttribute('role', 'dialog');
    this.#shell.elements.dialog?.setAttribute('aria-modal', 'true');
    this.#shell.elements.dialog?.setAttribute('aria-label', 'Add CivitAI API key');
    /** Contain keyboard interaction in the top modal so Escape leaves the underlying selection intact. */
    const keyboard = (event: KeyboardEvent): void => {
      event.stopPropagation();
      if (event.key === 'Escape') {
        event.preventDefault();
        if (!busy) this.#shell.close(false);
      } else if (event.key === 'Enter' && !(event.target instanceof HTMLButtonElement)) {
        event.preventDefault();
        if (!busy) this.#shell.elements.confirmButton?.click();
      } else if (event.key === 'Tab') {
        const controls = [
          ...(this.#shell.elements.dialog?.querySelectorAll<HTMLElement>(
            'a[href],input:not(:disabled),button:not(:disabled)',
          ) ?? []),
        ];
        const first = controls[0],
          last = controls.at(-1);
        if (event.shiftKey && this.#document.activeElement === first) {
          event.preventDefault();
          last?.focus();
        } else if (!event.shiftKey && this.#document.activeElement === last) {
          event.preventDefault();
          first?.focus();
        }
      }
    };
    this.#shell.elements.dialog?.addEventListener('keydown', keyboard);
    this.#shell.setConfirmEnabled(false);
    input.addEventListener('input', () =>
      this.#shell.setConfirmEnabled(!busy && !!input.value.trim()),
    );
    input.focus();
    try {
      return (await closed) === true;
    } finally {
      this.#shell.elements.dialog?.removeEventListener('keydown', keyboard);
      input.value = '';
      this.#controller?.abort();
      this.#controller = null;
      this.#shell.elements.overlay?.remove();
    }
  }

  /** Cancel secure entry when its originating picker disappears. */
  close(): void {
    this.#shell.close(false);
  }
}
