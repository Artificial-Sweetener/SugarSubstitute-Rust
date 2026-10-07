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
/** Own user cancellation eligibility, backdrop gestures, and nested modal ordering. */

interface DismissalOptions {
  allowEscapeClose?: boolean;
  allowOverlayClose?: boolean;
}

/** Keep all shell dismissal routes aligned while leaving rendering and result resolution to the shell. */
export class ModalDismissal {
  #busy = false;
  #backdropPress: boolean | null = null;

  /** Bind the shell's current elements and options without copying their lifecycle state. */
  constructor(
    private readonly document: Document | null,
    private readonly dialog: () => HTMLElement | undefined,
    private readonly options: () => DismissalOptions,
    private readonly cancel: () => void,
  ) {}

  /** Report the authoritative busy state for footer and header presentation. */
  get busy(): boolean {
    return this.#busy;
  }

  /** Prevent user cancellation while a transaction disables Cancel. */
  setBusy(busy: boolean): void {
    this.#busy = busy;
  }

  /** Forget gesture and busy state when the shell's lifetime ends. */
  reset(): void {
    this.#busy = false;
    this.#backdropPress = null;
  }

  /** Respect both stacked shells and contained dialogs using the same order as their overlays. */
  isFrontmost(): boolean {
    const dialogs = this.document?.querySelectorAll(
      '.sugarcubes-modal-overlay.is-visible [role="dialog"][aria-modal="true"]',
    );
    return dialogs?.item(dialogs.length - 1) === this.dialog();
  }

  /** Treat close and Cancel as explicit cancellation without invoking confirmation. */
  dismiss(): void {
    if (!this.#busy && this.isFrontmost()) this.cancel();
  }

  /** Remember where a press began so a drag out of the form cannot dismiss it. */
  press(event: Event): void {
    this.#backdropPress = event.target === event.currentTarget;
  }

  /** Dismiss only a plain backdrop click; content and suppressed pan clicks stay inside the dialog. */
  outside(event: Event): void {
    const eligible = this.#backdropPress !== false;
    this.#backdropPress = null;
    if (
      event.target !== event.currentTarget ||
      event.defaultPrevented ||
      !eligible ||
      this.options().allowOverlayClose === false
    )
      return;
    this.dismiss();
  }

  /** Consume Escape for the front dialog without letting the same key dismiss its parent. */
  escape(event: KeyboardEvent): void {
    if (this.options().allowEscapeClose === false) return;
    event.preventDefault();
    this.dismiss();
  }
}
