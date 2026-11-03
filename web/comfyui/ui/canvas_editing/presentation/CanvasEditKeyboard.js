const TEXT_ENTRY = 'input:not([type="range"]):not([type="checkbox"]):not([type="radio"]):not([type="button"]),' +
    'textarea,select,[contenteditable]:not([contenteditable="false"])';
/**
 * Listen at the document so shortcuts work while the native canvas holds the pointer, capturing
 * so canvas editing keys take precedence over the viewer's own navigation keys.
 *
 * A key belongs to the canvas only while focus rests in the canvas attachment or nowhere at all,
 * so graph, dialog and other panel shortcuts keep their owners. Text entry keeps its keys; the
 * editing chrome's own amount fields still pass Enter and Esc. Releasing Space, or the window
 * losing focus, always ends temporary navigation.
 */
export class CanvasEditKeyboard {
    document;
    scope;
    chrome;
    target;
    #window;
    /** A Space press the canvas took, whose release it must also take. */
    #holding = false;
    constructor(document, scope, chrome, target) {
        this.document = document;
        this.scope = scope;
        this.chrome = chrome;
        this.target = target;
        this.#window = document.defaultView;
        document.addEventListener('keydown', this.#down, true);
        document.addEventListener('keyup', this.#up, true);
        this.#window?.addEventListener('blur', this.#blur);
    }
    dispose() {
        this.document.removeEventListener('keydown', this.#down, true);
        this.document.removeEventListener('keyup', this.#up, true);
        this.#window?.removeEventListener('blur', this.#blur);
    }
    #down = (event) => this.#route(event, 'down');
    #up = (event) => this.#route(event, 'up');
    #blur = () => {
        this.#holding = false;
        this.target()?.releaseNavigation();
    };
    #route(event, phase) {
        const target = this.target();
        if (!target || event.defaultPrevented || event.isComposing)
            return;
        const space = event.key === ' ';
        const releasing = phase === 'up' && space && this.#holding;
        if (!releasing && !(this.#focused() && this.#admits(event)))
            return;
        const consumed = target.key({
            phase,
            key: event.key,
            ctrl: event.ctrlKey,
            meta: event.metaKey,
            shift: event.shiftKey,
            alt: event.altKey,
        });
        if (space)
            this.#holding = phase === 'down' && consumed;
        if (!consumed)
            return;
        event.preventDefault();
        event.stopPropagation();
    }
    /** Report whether keyboard focus rests in the canvas attachment or on no element. */
    #focused() {
        const active = this.document.activeElement;
        return (!active ||
            active === this.document.body ||
            active === this.document.documentElement ||
            this.scope.contains(active));
    }
    /**
     * Leave typing to text fields and activation keys to the chrome's own controls.
     *
     * Within the editing chrome only Esc, and Enter in its amount fields, reach the canvas, so
     * Enter and Space still press a focused button. Outside text entry every shortcut applies.
     */
    #admits(event) {
        const origin = event.target;
        if (!(origin instanceof Element))
            return true;
        const typing = origin.closest(TEXT_ENTRY) !== null;
        if (!this.chrome.contains(origin))
            return !typing;
        return event.key === 'Escape' || (typing && event.key === 'Enter');
    }
}
