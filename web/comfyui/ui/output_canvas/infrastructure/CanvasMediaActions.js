import { canvasMediaItem } from '../domain/CanvasMedia.js';
/** Keep late media requests from acting on a replacement workflow, item or newer copy intent. */
export class CanvasMediaActions {
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
    async run(command, scope) {
        if (this.#disposed || !this.options.accepts() || !scope.current())
            return false;
        const item = canvasMediaItem(this.options.read(), command.target);
        if (!item)
            return false;
        return await this.#perform(() => canvasMediaItem(this.options.read(), command.target)?.id === item.id, scope, async (accepts, signal) => command.type === 'copy-media'
            ? ((await this.options.copy?.(item, accepts, command.format, signal)) ?? false)
            : ((await this.options.fileAction?.(item, accepts, signal)) ?? false));
    }
    /**
     * Apply the file action to the exact selected output of any media kind.
     *
     * Native presentation opens outputs in place of a browser window through this action.
     */
    async open(item, scope) {
        const fileAction = this.options.fileAction;
        if (this.#disposed || !fileAction || !this.options.accepts() || !scope.current())
            return false;
        const selected = () => {
            const snapshot = this.options.read();
            return snapshot.selected?.id === item.id || snapshot.secondary?.id === item.id;
        };
        if (!selected())
            return false;
        return await this.#perform(selected, scope, (accepts, signal) => fileAction(item, accepts, signal));
    }
    /** Run one operation that supersedes earlier ones while its pinned output remains current. */
    async #perform(current, scope, run) {
        this.#pending?.controller.abort();
        const controller = new AbortController();
        this.#pending = { controller, current };
        const operation = ++this.#operation;
        const cancel = () => controller.abort();
        scope.signal.addEventListener('abort', cancel, { once: true });
        if (scope.signal.aborted)
            cancel();
        const accepts = () => !this.#disposed &&
            !controller.signal.aborted &&
            scope.current() &&
            operation === this.#operation &&
            this.options.accepts() &&
            current();
        try {
            if (!accepts())
                return false;
            return await run(accepts, controller.signal);
        }
        catch (error) {
            if (controller.signal.aborted && error instanceof DOMException && error.name === 'AbortError')
                return false;
            throw error;
        }
        finally {
            scope.signal.removeEventListener('abort', cancel);
            if (this.#pending?.controller === controller)
                this.#pending = null;
        }
    }
    /** Cancel transport when a published state retires the exact originating output. */
    reconcile() {
        const pending = this.#pending;
        if (pending && (!this.options.accepts() || !pending.current()))
            pending.controller.abort();
    }
    /** Invalidate pending work before the attachment and its resources retire. */
    dispose() {
        this.#disposed = true;
        this.#pending?.controller.abort();
        this.#operation++;
    }
}
