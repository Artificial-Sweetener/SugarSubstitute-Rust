const typography = [
    'font-family',
    'font-size',
    'font-weight',
    'font-style',
    'font-variant',
    'line-height',
    'letter-spacing',
    'word-spacing',
    'text-align',
    'text-indent',
    'text-transform',
    'direction',
    'tab-size',
    'padding-top',
    'padding-right',
    'padding-bottom',
    'padding-left',
    'border-top-width',
    'border-right-width',
    'border-bottom-width',
    'border-left-width',
    'border-radius',
    'word-break',
    'overflow-wrap',
];
/** Own only highlight layers for current matches and release every reference on surface teardown. */
export class SearchHighlights {
    #bindings = new Map();
    #observer;
    #document;
    #frame = null;
    #active = null;
    /** Observe matched native geometry without discovering graph candidates or following animation frames. */
    constructor(documentRef) {
        this.#document = documentRef;
        const Observer = documentRef.defaultView?.ResizeObserver;
        this.#observer = Observer ? new Observer(() => this.#schedule()) : null;
    }
    /** Reconcile marks using literal text nodes and retain unchanged controls across navigation. */
    render(fields, matches, active) {
        const created = [];
        const grouped = new Map();
        for (const match of matches) {
            const ranges = grouped.get(match.field) ?? [];
            ranges.push(match);
            grouped.set(match.field, ranges);
        }
        for (const [id, binding] of this.#bindings) {
            const field = fields.get(id);
            if (!grouped.has(id) || field?.control !== binding.control)
                this.#release(id, binding);
        }
        const parentPositions = new Map();
        for (const id of grouped.keys()) {
            const parent = fields.get(id)?.control?.parentElement;
            if (parent && !this.#bindings.has(id) && !parentPositions.has(parent))
                parentPositions.set(parent, getComputedStyle(parent).position);
        }
        for (const [id, ranges] of grouped) {
            const field = fields.get(id);
            const control = field?.control;
            const parent = control?.parentElement;
            if (!field || !control || !parent)
                continue;
            let binding = this.#bindings.get(id);
            const signature = JSON.stringify([
                control.value,
                ranges.map((range) => [range.start, range.length]),
            ]);
            if (!binding) {
                const position = parent.style.getPropertyValue('position');
                const positionPriority = parent.style.getPropertyPriority('position');
                const computedPosition = parentPositions.get(parent);
                if (computedPosition === 'static' || !computedPosition)
                    parent.style.setProperty('position', 'relative');
                const layer = this.#document.createElement('div');
                layer.className = 'sc-search-highlight';
                layer.setAttribute('aria-hidden', 'true');
                const text = this.#document.createElement('div');
                text.className = 'sc-search-highlight__text';
                const marks = new Map();
                layer.append(text);
                const scroll = () => {
                    text.style.transform = `translate(${-control.scrollLeft}px,${-control.scrollTop}px)`;
                };
                binding = {
                    field,
                    control,
                    parent,
                    position,
                    positionPriority,
                    layer,
                    text,
                    marks,
                    scroll,
                };
                parent.append(layer);
                control.addEventListener('scroll', scroll, { passive: true });
                this.#bindings.set(id, binding);
                this.#observer?.observe(control);
                created.push(binding);
            }
            if (binding.layer.dataset.ranges !== signature) {
                binding.layer.dataset.ranges = signature;
                binding.marks.clear();
                const fragments = [];
                let offset = 0;
                for (const range of ranges) {
                    fragments.push(this.#document.createTextNode(control.value.slice(offset, range.start)));
                    const mark = this.#document.createElement('mark');
                    mark.textContent = control.value.slice(range.start, range.start + range.length);
                    binding.marks.set(range.start, mark);
                    fragments.push(mark);
                    offset = range.start + range.length;
                }
                fragments.push(this.#document.createTextNode(control.value.slice(offset) + '\u200b'));
                binding.text.replaceChildren(...fragments);
            }
        }
        if (created.length)
            this.#geometry(created);
        this.activate(active);
    }
    /** Update only the old and new active fragments when navigating cached results. */
    activate(active) {
        const mark = active
            ? (this.#bindings.get(active.field)?.marks.get(active.start) ?? null)
            : null;
        if (this.#active === mark)
            return;
        this.#active?.removeAttribute('data-active');
        mark?.setAttribute('data-active', '');
        this.#active = mark;
    }
    /** Reveal the active fragment inside a scrolling native control while leaving finder focus intact. */
    reveal(match) {
        const binding = this.#bindings.get(match.field);
        const mark = binding?.marks.get(match.start);
        if (!binding || !mark)
            return;
        const { control } = binding;
        const fragment = mark.getBoundingClientRect();
        const bounds = control.getBoundingClientRect();
        if (fragment.top < bounds.top)
            control.scrollTop -= bounds.top - fragment.top + 4;
        else if (fragment.bottom > bounds.bottom)
            control.scrollTop += fragment.bottom - bounds.bottom + 4;
        if (fragment.left < bounds.left)
            control.scrollLeft -= bounds.left - fragment.left + 4;
        else if (fragment.right > bounds.right && control instanceof HTMLInputElement)
            control.scrollLeft += fragment.right - bounds.right + 4;
        binding.scroll();
    }
    /** Remove marks without moving native editing cursors. */
    clear() {
        this.activate(null);
        for (const [id, binding] of this.#bindings)
            this.#release(id, binding);
    }
    /** Stop all owned callbacks before borrowed native controls return to graph mode. */
    dispose() {
        if (this.#frame !== null)
            this.#document.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.clear();
        this.#observer?.disconnect();
    }
    /** Copy native typography and local dimensions so wrapped highlights share the real widget layout. */
    #geometry(bindings = this.#bindings.values()) {
        const measurements = [...bindings].map((binding) => {
            const { control } = binding;
            const style = getComputedStyle(control);
            return {
                binding,
                typography: typography.map((property) => [property, style.getPropertyValue(property)]),
                left: control.offsetLeft,
                top: control.offsetTop,
                width: control.offsetWidth,
                height: control.offsetHeight,
                x: control.scrollLeft,
                y: control.scrollTop,
            };
        });
        for (const measurement of measurements) {
            const { control, layer, text } = measurement.binding;
            for (const [property, value] of measurement.typography)
                layer.style.setProperty(property, value);
            layer.style.boxSizing = 'border-box';
            layer.style.borderStyle = 'solid';
            layer.style.borderColor = 'transparent';
            layer.style.left = `${measurement.left}px`;
            layer.style.top = `${measurement.top}px`;
            layer.style.width = `${measurement.width}px`;
            layer.style.height = `${measurement.height}px`;
            text.style.whiteSpace =
                control instanceof HTMLTextAreaElement && control.wrap !== 'off' ? 'pre-wrap' : 'pre';
            text.style.transform = `translate(${-measurement.x}px,${-measurement.y}px)`;
        }
    }
    /** Coalesce geometry changes independently of query matching and native field lookup. */
    #schedule() {
        if (this.#frame !== null)
            return;
        const windowRef = this.#document.defaultView;
        if (!windowRef)
            return;
        this.#frame = windowRef.requestAnimationFrame(() => {
            this.#frame = null;
            this.#geometry();
        });
    }
    /** Restore only presentation properties owned by this matched control. */
    #release(id, binding) {
        this.#observer?.unobserve(binding.control);
        binding.control.removeEventListener('scroll', binding.scroll);
        binding.layer.remove();
        if (binding.position)
            binding.parent.style.setProperty('position', binding.position, binding.positionPriority);
        else
            binding.parent.style.removeProperty('position');
        this.#bindings.delete(id);
    }
}
