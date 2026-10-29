/** Bind only discrete keyboard intent; native pointer gestures remain with their host. */
export class CanvasViewportKeyboard {
    element;
    port;
    report;
    cancel;
    #disposed = false;
    /** Retain the existing port identity, capability checks and asynchronous cancellation owner. */
    constructor(element, port, report, cancel) {
        this.element = element;
        this.port = port;
        this.report = report;
        this.cancel = cancel;
        element.tabIndex = 0;
        element.setAttribute('role', 'region');
        element.setAttribute('aria-label', 'Output image viewer');
        element.addEventListener('keydown', this.#key);
    }
    /** Remove only this keyboard binding; the attachment owns in-flight commands. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.element.removeEventListener('keydown', this.#key);
    }
    /** Preserve editable descendants and host/browser modifier shortcuts. */
    #key = (event) => {
        if (this.#disposed ||
            event.target !== this.element ||
            event.ctrlKey ||
            event.metaKey ||
            event.altKey)
            return;
        if (event.key === 'Escape' && this.cancel) {
            this.cancel();
        }
        else {
            const command = this.#command(event);
            if (!command || !this.port.capabilities.commands.includes(command.type))
                return;
            if (this.port.read().preview) {
                event.preventDefault();
                event.stopPropagation();
                return;
            }
            let request = null;
            try {
                request = this.port.createRequest(command, this.port.read().revision);
                const admitted = request;
                void this.port.dispatch(admitted).catch((error) => this.report(error, admitted));
            }
            catch (error) {
                this.report(error, request);
            }
        }
        event.preventDefault();
        event.stopPropagation();
    };
    /** Express arrows as viewport fractions while preserving the existing forty-CSS-pixel step. */
    #command(event) {
        if (event.key === '+' || event.key === '=')
            return { type: 'zoom', factor: 1.25 };
        if (event.key === '-')
            return { type: 'zoom', factor: 0.8 };
        if (event.shiftKey)
            return null;
        if (event.key === '0')
            return { type: 'fit' };
        if (event.key === '1')
            return { type: 'native' };
        const bounds = this.element.getBoundingClientRect();
        if (!(bounds.width > 0 && bounds.height > 0))
            return null;
        const x = Math.min(10, 40 / bounds.width);
        const y = Math.min(10, 40 / bounds.height);
        switch (event.key) {
            case 'ArrowLeft':
                return { type: 'pan', x, y: 0, units: 'viewport' };
            case 'ArrowRight':
                return { type: 'pan', x: -x, y: 0, units: 'viewport' };
            case 'ArrowUp':
                return { type: 'pan', x: 0, y, units: 'viewport' };
            case 'ArrowDown':
                return { type: 'pan', x: 0, y: -y, units: 'viewport' };
            default:
                return null;
        }
    }
}
