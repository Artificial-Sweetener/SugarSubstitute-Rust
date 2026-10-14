import { canvasMediaItem } from '../domain/CanvasMedia.js';
/** Keep late media requests from acting on a replacement workflow, item or newer copy intent. */
export class BrowserCanvasMediaActions {
    options;
    #operation = 0;
    #disposed = false;
    #pending = null;
    /** Bind the live renderer-neutral state and explicit platform action ports. */
    constructor(options) {
        this.options = options;
    }
    /** Advertise only platform-supported commands without teaching menus about browser APIs. */
    commands() {
        return [
            ...(this.options.copy ? ['copy-media'] : []),
            ...(this.options.fileAction ? ['file-media'] : []),
        ];
    }
    /** Publish immutable shared action metadata without network requests or image-resource ownership. */
    profile() {
        return this.options.profile?.() ?? null;
    }
    /** Execute a pinned file action and revalidate identity before asynchronous clipboard publication. */
    async run(command) {
        if (this.#disposed || !this.options.accepts())
            return false;
        const item = canvasMediaItem(this.options.read(), command.target);
        if (!item)
            return false;
        this.#pending?.controller.abort();
        const controller = new AbortController();
        this.#pending = { controller, target: command.target, item };
        const operation = ++this.#operation;
        const accepts = () => !this.#disposed &&
            !controller.signal.aborted &&
            operation === this.#operation &&
            this.options.accepts() &&
            canvasMediaItem(this.options.read(), command.target)?.id === item.id;
        try {
            return command.type === 'copy-media'
                ? ((await this.options.copy?.(item, accepts, command.format, controller.signal)) ?? false)
                : ((await this.options.fileAction?.(item, accepts, controller.signal)) ?? false);
        }
        catch (error) {
            if (controller.signal.aborted && error instanceof DOMException && error.name === 'AbortError')
                return false;
            throw error;
        }
        finally {
            if (this.#pending?.controller === controller)
                this.#pending = null;
        }
    }
    /** Cancel transport when a published state retires the exact originating image. */
    reconcile() {
        const pending = this.#pending;
        if (pending &&
            (!this.options.accepts() ||
                canvasMediaItem(this.options.read(), pending.target)?.id !== pending.item.id))
            pending.controller.abort();
    }
    /** Invalidate pending work before the attachment and its resources retire. */
    dispose() {
        this.#disposed = true;
        this.#pending?.controller.abort();
        this.#operation++;
    }
}
