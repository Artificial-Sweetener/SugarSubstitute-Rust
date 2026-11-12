/** Retain action identity for unchanged thumbnails and reflow only this mounted preview. */
export class ComfyImageRevealDom {
    root;
    options;
    #buttons = new Map();
    #resize;
    #frame = null;
    /** Observe native geometry independently of graph inventory and render clocks. */
    constructor(root, options) {
        this.root = root;
        this.options = options;
        this.#resize =
            typeof ResizeObserver === 'function' ? new ResizeObserver(() => this.#schedule()) : null;
        this.#resize?.observe(root);
        root.addEventListener('contextmenu', this.#context, true);
        this.refresh();
    }
    /** Reconcile only changed media and preserve all surviving native and injected control identities. */
    refresh() {
        const images = new Set(this.root.querySelectorAll('img'));
        for (const [image, binding] of this.#buttons) {
            const target = images.has(image) ? this.options.target(image) : null;
            if (!target ||
                target.itemId !== binding.target.itemId ||
                target.generation !== binding.target.generation) {
                binding.button.remove();
                this.#buttons.delete(image);
            }
        }
        for (const image of images) {
            if (this.#buttons.has(image))
                continue;
            const target = this.options.target(image);
            if (!target)
                continue;
            const button = createImageRevealButton(this.root.ownerDocument, () => this.options.reveal(target));
            this.#buttons.set(image, { button, target });
            this.root.append(button);
        }
        this.#schedule();
    }
    /** Release scoped geometry and actions without modifying native preview layout. */
    dispose() {
        this.root.removeEventListener('contextmenu', this.#context, true);
        this.#resize?.disconnect();
        if (this.#frame !== null)
            this.root.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
        for (const { button } of this.#buttons.values())
            button.remove();
        this.#buttons.clear();
    }
    /** Capture exact gallery media before Comfy constructs its normal node context menu. */
    #context = (event) => {
        const element = event.target instanceof Element ? event.target : null;
        const image = element instanceof HTMLImageElement
            ? element
            : (element?.closest('button')?.querySelector('img') ?? this.root.querySelector('img'));
        if (image instanceof HTMLImageElement)
            this.options.context(image);
    };
    /** Coalesce local image replacement and resize notifications into one settled geometry update. */
    #schedule() {
        if (this.#frame !== null)
            return;
        const window = this.root.ownerDocument.defaultView;
        if (!window)
            return;
        this.#frame = window.requestAnimationFrame(() => {
            this.#frame = null;
            this.#layout();
        });
    }
    /** Place sibling overlay buttons over native cells, never nested inside Comfy's gallery buttons. */
    #layout() {
        const root = this.root.getBoundingClientRect();
        const scale = this.root.offsetWidth ? root.width / this.root.offsetWidth : 1;
        if (!scale)
            return;
        for (const [image, { button }] of this.#buttons) {
            const holder = image.closest('button') ?? image.parentElement;
            if (!holder)
                continue;
            const rect = holder.getBoundingClientRect();
            const actions = holder.querySelector('.actions');
            const controls = actions?.offsetWidth ?? 0;
            button.style.left = `${Math.max(0, (rect.right - root.left) / scale - 32 - 8 - (controls ? controls + 8 : 0))}px`;
            button.style.top = `${Math.max(0, (rect.top - root.top) / scale + 8)}px`;
        }
    }
}
/** Build an accessible action using the installed host icon font and explicit event isolation. */
export function createImageRevealButton(document, reveal) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'sc-image-reveal';
    button.dataset.scImageReveal = '';
    button.setAttribute('aria-label', 'Show in output canvas');
    button.title = 'Show in output canvas';
    const icon = document.createElement('i');
    icon.className = 'pi pi-window-maximize';
    icon.setAttribute('aria-hidden', 'true');
    button.append(icon);
    button.addEventListener('pointerdown', (event) => event.stopPropagation());
    button.addEventListener('click', (event) => {
        event.preventDefault();
        event.stopPropagation();
        reveal();
    });
    return button;
}
/** Install scoped hover/focus styles without affecting native preview geometry or controls. */
export function ensureImageRevealStyles(document) {
    if (document.getElementById('sc-image-reveal-styles'))
        return;
    const style = document.createElement('style');
    style.id = 'sc-image-reveal-styles';
    style.textContent = `.sc-image-reveal { position:absolute; z-index:4; display:grid; place-items:center; width:32px; height:32px; padding:0; border:0; border-radius:8px; background:var(--fg-color,#f2f2f2); color:var(--bg-color,#151515); box-shadow:0 2px 8px #0005; cursor:pointer; opacity:0; pointer-events:none; transition:opacity 120ms ease; }
  .image-preview:hover > .sc-image-reveal,.image-preview:focus-within > .sc-image-reveal,.sugarcubes-cube-face__preview-media figure:hover > .sc-image-reveal,.sugarcubes-cube-face__preview-media figure:focus-within > .sc-image-reveal,.sc-image-reveal:focus-visible { opacity:1;pointer-events:auto; }
  .sc-image-reveal:focus-visible { outline:2px solid var(--p-primary-color,#409eff);outline-offset:2px; }
  .sc-image-reveal-legacy {position:fixed;opacity:1;pointer-events:auto;z-index:1100;}`;
    document.head.append(style);
}
