// frontend/comfyui/ui/dialogs/ModalDismissal.ts
var ModalDismissal = class {
  /** Bind the shell's current elements and options without copying their lifecycle state. */
  constructor(document, dialog, options, cancel) {
    this.document = document;
    this.dialog = dialog;
    this.options = options;
    this.cancel = cancel;
  }
  #busy = false;
  #backdropPress = null;
  /** Report the authoritative busy state for footer and header presentation. */
  get busy() {
    return this.#busy;
  }
  /** Prevent user cancellation while a transaction disables Cancel. */
  setBusy(busy) {
    this.#busy = busy;
  }
  /** Forget gesture and busy state when the shell's lifetime ends. */
  reset() {
    this.#busy = false;
    this.#backdropPress = null;
  }
  /** Respect both stacked shells and contained dialogs using the same order as their overlays. */
  isFrontmost() {
    const dialogs = this.document?.querySelectorAll(
      '.sugarcubes-modal-overlay.is-visible [role="dialog"][aria-modal="true"]'
    );
    return dialogs?.item(dialogs.length - 1) === this.dialog();
  }
  /** Treat close and Cancel as explicit cancellation without invoking confirmation. */
  dismiss() {
    if (!this.#busy && this.isFrontmost()) this.cancel();
  }
  /** Remember where a press began so a drag out of the form cannot dismiss it. */
  press(event) {
    this.#backdropPress = event.target === event.currentTarget;
  }
  /** Dismiss only a plain backdrop click; content and suppressed pan clicks stay inside the dialog. */
  outside(event) {
    const eligible = this.#backdropPress !== false;
    this.#backdropPress = null;
    if (event.target !== event.currentTarget || event.defaultPrevented || !eligible || this.options().allowOverlayClose === false)
      return;
    this.dismiss();
  }
  /** Consume Escape for the front dialog without letting the same key dismiss its parent. */
  escape(event) {
    if (this.options().allowEscapeClose === false) return;
    event.preventDefault();
    this.dismiss();
  }
};

export {
  ModalDismissal
};
