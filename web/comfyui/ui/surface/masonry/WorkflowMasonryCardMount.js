import { NativeNodeCardHost } from '../NativeNodeCardHost.js';
import { prepareNativeCardPresentation, } from '../NativeCardPresentationOwner.js';
/** Release only presentation resources owned here, keeping source native widgets alive. */
export class WorkflowMasonryCardMount {
    #options;
    #card;
    #transfer;
    #host = null;
    #disposed = false;
    #mounted = false;
    /** Prepare source reads separately from the later DOM relocation or native mount. */
    constructor(options) {
        this.#options = options;
        this.#card = options.card;
        this.#transfer = options.source
            ? prepareNativeCardPresentation(options.source, this.#onInvalidated)
            : null;
    }
    /** Budget frame yields for newly rendered controls rather than cheap native cell transfers. */
    get requiresNativeMount() {
        return this.#transfer === null;
    }
    /** Attach one native control tree, preferring its existing Cube-face presentation. */
    mount() {
        if (this.#disposed || this.#mounted)
            return;
        this.#mounted = true;
        if (this.#transfer)
            this.#transfer.move(this.#options.cell);
        else {
            this.#host = new NativeNodeCardHost(this.#options.renderer);
            this.#host.mountCard(this.#options.cell, this.#card, this.#options.onActivationChange, {
                allowCollapse: this.#card.prompt !== true,
            });
        }
    }
    /** Refresh shared activation data without rebuilding controls or replacing a borrowed renderer. */
    refresh(card) {
        this.#card = card;
        this.#host?.refresh([card]);
        const cell = this.#transfer?.element ?? this.#options.cell;
        cell.dataset.cubeColumnSpan = String(card.columnSpan ?? 1);
    }
    /** Return a borrowed native cell or release a temporary native projection. */
    dispose() {
        this.#disposed = true;
        this.#transfer?.restore();
        this.#transfer = null;
        this.#host?.dispose();
        this.#host = null;
    }
    /** Reacquire only this card after its source owner replaces controls during a policy change. */
    #onInvalidated = () => {
        queueMicrotask(() => {
            if (this.#disposed)
                return;
            const source = this.#options.getSource();
            this.#transfer = source ? prepareNativeCardPresentation(source, this.#onInvalidated) : null;
            this.#mounted = false;
            this.mount();
            this.#options.onRemounted();
        });
    };
}
