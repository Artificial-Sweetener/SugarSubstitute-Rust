import { ModelVersionDialog } from './ModelVersionDialog.js';
/** Own field-scoped transfer feedback while resolving each commit against current host choices. */
export class ModelPickerSession {
    #document;
    #resolve;
    #listeners = new Set();
    #progress = null;
    #dialog = null;
    #active = true;
    #transfer = 0;
    #controller = null;
    /** Bind a receiver that survives presentation replacement but rejects removed workflow fields. */
    constructor(document, resolve) {
        this.#document = document;
        this.#resolve = resolve;
    }
    /** Read the current workflow choice independently of the inspected model family. */
    current() {
        return this.#resolve()?.readValue() ?? '';
    }
    /** Expose the current transfer without allowing views to own its state. */
    read() {
        return this.#progress;
    }
    /** Follow progress until the temporary field renderer is replaced or removed. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Write only an exact legal host option, preserving host path separator conventions. */
    select(filename) {
        const binding = this.#active ? this.#resolve() : null;
        const legal = binding?.options.find((option) => option.replaceAll('\\', '/') === filename.replaceAll('\\', '/'));
        if (!binding || legal === undefined || binding.disabled())
            throw new Error('The model is installed, but its current Comfy field is unavailable.');
        binding.writeValue(legal);
    }
    /** Publish one scoped transfer and clear it on success, cancellation, or failure. */
    async download(operation, model, signal) {
        if (!this.#active || this.#progress)
            throw new Error('The model field is unavailable or already downloading.');
        const transfer = ++this.#transfer;
        const controller = new AbortController();
        this.#controller = controller;
        const cancel = () => controller.abort();
        signal?.addEventListener('abort', cancel, { once: true });
        if (signal?.aborted)
            cancel();
        this.#publish({ message: 'Waiting to download…', percentage: null, model });
        try {
            if (controller.signal.aborted)
                throw new DOMException('Cancelled', 'AbortError');
            const filename = await operation((progress) => {
                if (this.#active && !controller.signal.aborted && transfer === this.#transfer)
                    this.#publish({ ...progress, model });
            }, controller.signal);
            if (!this.#active || controller.signal.aborted)
                throw new DOMException('Cancelled', 'AbortError');
            return filename;
        }
        finally {
            signal?.removeEventListener('abort', cancel);
            if (this.#controller === controller)
                this.#controller = null;
            if (transfer === this.#transfer) {
                this.#transfer += 1;
                this.#publish(null);
            }
        }
    }
    /** Keep an accepted acquisition alive until its refreshed legal choice is committed or its field disappears. */
    async useDownload(operation, model) {
        try {
            this.select(await this.download(operation, model));
        }
        catch (error) {
            if (this.#active)
                throw error;
        }
    }
    /** Browse versions without tying the modal to a temporary combobox renderer. */
    async versions(item, actions, feedback) {
        if (!this.#active || this.#progress)
            return;
        this.#dialog?.close();
        const dialog = new ModelVersionDialog(this.#document, item, actions, this, feedback);
        this.#dialog = dialog;
        try {
            await dialog.open();
        }
        finally {
            if (this.#dialog === dialog)
                this.#dialog = null;
        }
    }
    /** Close interactions when the actual node or supported renderer disappears. */
    dispose() {
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
    #publish(progress) {
        this.#progress = progress;
        for (const listener of this.#listeners)
            listener();
    }
}
