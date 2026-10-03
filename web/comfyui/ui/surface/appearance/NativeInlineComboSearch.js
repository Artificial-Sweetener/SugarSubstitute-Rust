const geometryProperties = ['left', 'top', 'width', 'height', 'font'];
/** Own the reversible geometry of one native searchable combobox, without duplicating its state. */
export class NativeInlineComboSearch {
    #binding = null;
    #frame = null;
    #document;
    /** Follow scrolling, graph transforms, and popup placement only while a search is open. */
    constructor(documentRef) {
        this.#document = documentRef;
    }
    /** Project only the positively identified native default-combobox input into its original field. */
    present(trigger, popup) {
        if (this.#binding?.input.isConnected && this.#binding.popup === popup)
            return;
        this.clear();
        if (trigger.dataset.testid !== 'widget-select-default-trigger')
            return;
        const input = popup.querySelector('[data-testid="widget-select-default-search-input"]');
        if (!(input instanceof HTMLInputElement) || !input.parentElement)
            return;
        const shell = input.parentElement;
        this.#binding = { trigger, popup, input, shell, label: input.getAttribute('aria-label') };
        input.setAttribute('data-sc-inline-search', '');
        shell.setAttribute('data-sc-inline-search-shell', '');
        input.setAttribute('aria-label', trigger.getAttribute('aria-label') ?? 'Search');
        this.#layout();
        this.#schedule();
    }
    /** Release styles before theme suspension or after the host closes its popup. */
    clear() {
        const binding = this.#binding;
        this.#binding = null;
        if (this.#frame !== null)
            this.#document.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        if (!binding)
            return;
        binding.input.removeAttribute('data-sc-inline-search');
        binding.shell.removeAttribute('data-sc-inline-search-shell');
        if (binding.label === null)
            binding.input.removeAttribute('aria-label');
        else
            binding.input.setAttribute('aria-label', binding.label);
        for (const property of geometryProperties)
            binding.input.style.removeProperty(`--sc-search-${property}`);
    }
    /** Stop tracking a detached search input without retaining its host component tree. */
    reconcile() {
        if (this.#binding && (!this.#binding.input.isConnected || !this.#binding.trigger.isConnected))
            this.clear();
    }
    /** Coalesce geometry reads and follow Comfy's compositor-driven graph pan and zoom. */
    #schedule = () => {
        if (!this.#binding || this.#frame !== null)
            return;
        this.#frame =
            this.#document.defaultView?.requestAnimationFrame(() => {
                this.#frame = null;
                this.reconcile();
                if (!this.#binding)
                    return;
                this.#layout();
                this.#schedule();
            }) ?? null;
    };
    /** Keep the native search field precisely within the trigger's screen bounds. */
    #layout() {
        const binding = this.#binding;
        if (!binding)
            return;
        const trigger = binding.trigger.getBoundingClientRect();
        const popup = binding.popup.getBoundingClientRect();
        const field = binding.trigger.closest('[data-capture-wheel]') ?? binding.trigger;
        const fieldRectangle = field.getBoundingClientRect();
        const scale = fieldRectangle.height / Math.max(1, field.offsetHeight);
        const popupStyle = this.#document.defaultView?.getComputedStyle(binding.popup);
        const font = Number.parseFloat(this.#document.defaultView?.getComputedStyle(binding.trigger).fontSize || '12');
        const values = {
            left: fieldRectangle.left - popup.left - Number.parseFloat(popupStyle?.borderLeftWidth || '0'),
            top: fieldRectangle.top - popup.top - Number.parseFloat(popupStyle?.borderTopWidth || '0'),
            width: trigger.width,
            height: fieldRectangle.height,
            font: font * scale,
        };
        for (const property of geometryProperties) {
            const value = `${String(values[property])}px`;
            const name = `--sc-search-${property}`;
            if (binding.input.style.getPropertyValue(name) !== value)
                binding.input.style.setProperty(name, value);
        }
    }
}
