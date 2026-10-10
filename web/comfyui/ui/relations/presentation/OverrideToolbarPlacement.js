/** Reconcile host placement and settled sizes without observing disclosure animation frames. */
export class OverrideToolbarPlacement {
    root;
    grip;
    host;
    changed;
    settled;
    store;
    #docked = false;
    #requestedDocked;
    #unsubscribeLayout;
    #parent = null;
    #observer = null;
    #frame = null;
    #desired = null;
    #applied = { x: 0, y: 0 };
    #size = { width: 0, height: 0 };
    #gesture = null;
    #pointerPlaced = false;
    #retryDock = false;
    /** Bind the host boundary and publish placement changes to disclosure presentation. */
    constructor(root, grip, host, changed, settled, store) {
        this.root = root;
        this.grip = grip;
        this.host = host;
        this.changed = changed;
        this.settled = settled;
        this.store = store;
        const saved = store.read();
        this.#requestedDocked = saved.docked;
        this.#desired = saved.floating;
        this.#unsubscribeLayout = host.subscribeLayout(this.#hostResized);
        root.dataset.placement = 'floating';
    }
    /** Reattach after native shell replacement, preserving the requested resting placement. */
    attached() {
        this.#setDocked(this.#requestedDocked);
        if (!this.host.attach(this.root, this.#docked)) {
            if (!this.#docked)
                return false;
            this.#setDocked(false);
            if (!this.host.attach(this.root, false))
                return false;
        }
        const parent = this.root.closest('.p-splitterpanel') ?? this.root.parentElement;
        if (this.#parent !== parent) {
            this.#observer?.disconnect();
            this.#observer = null;
            this.#parent = parent;
            const Observer = this.root.ownerDocument.defaultView?.ResizeObserver;
            if (Observer && parent) {
                this.#observer = new Observer(this.#hostResized);
                this.#observer.observe(parent);
            }
        }
        this.#place();
        this.#schedule();
        return true;
    }
    /** Clamp against the disclosure destination rather than its moving intermediate bounds. */
    resize(width, height) {
        this.#size = { width, height };
        this.#place();
    }
    /** Capture the pointer's position inside the grip before either placement can change. */
    begin(x, y) {
        const grip = this.grip.getBoundingClientRect();
        this.#gesture = { start: { x, y }, grip: { x: x - grip.left, y: y - grip.top }, moved: false };
        const bounds = this.root.getBoundingClientRect();
        this.resize(bounds.width, bounds.height);
    }
    /** Undock after deliberate movement and follow the grip without translating popup containers. */
    move(x, y) {
        const gesture = this.#gesture;
        if (!gesture || (!gesture.moved && Math.hypot(x - gesture.start.x, y - gesture.start.y) < 4))
            return;
        gesture.moved = true;
        this.#pointerPlaced = true;
        this.#requestedDocked = false;
        if (this.#docked) {
            this.#setDocked(false);
            this.attached();
            const bounds = this.root.getBoundingClientRect();
            this.#size = { width: bounds.width, height: bounds.height };
        }
        this.host.dragging(true);
        const root = this.root.getBoundingClientRect(), grip = this.grip.getBoundingClientRect();
        this.#desired = {
            x: x - gesture.grip.x - (grip.left - root.left),
            y: y - gesture.grip.y - (grip.top - root.top),
        };
        this.#place();
        this.host.preview(this.#nearDock(x, y));
    }
    /** Dock only a completed drop inside the dedicated placeholder. */
    end(x, y, accepted) {
        const gesture = this.#gesture;
        this.#gesture = null;
        const docked = accepted && gesture && gesture.moved && this.#nearDock(x, y);
        this.host.dragging(false);
        if (docked) {
            this.#requestedDocked = true;
            this.#setDocked(true);
            this.attached();
        }
        if (accepted && gesture?.moved)
            this.#save();
    }
    /** Offer keyboard docking without requiring a pointer gesture. */
    toggleDock() {
        if (!this.#docked && !this.host.canDock())
            return;
        this.#requestedDocked = !this.#docked;
        this.#setDocked(!this.#docked);
        this.attached();
        this.#save();
    }
    /** Move an undocked toolbar through the same bounded coordinate owner. */
    shift(x, y) {
        this.#requestedDocked = false;
        if (this.#docked) {
            this.#setDocked(false);
            this.attached();
        }
        this.#desired = { x: this.#applied.x + x, y: this.#applied.y + y };
        this.#place();
        this.#save();
    }
    /** Restore the default top dock without changing disclosure or widget values. */
    reset() {
        this.#requestedDocked = true;
        this.#desired = null;
        this.#pointerPlaced = false;
        this.attached();
        this.#save();
    }
    /** Release host chrome, geometry observation, and queued work at toolbar teardown. */
    dispose() {
        this.#unsubscribeLayout();
        this.#observer?.disconnect();
        if (this.#frame !== null)
            this.root.ownerDocument.defaultView?.cancelAnimationFrame(this.#frame);
        this.#frame = null;
        this.#parent = null;
        this.#gesture = null;
        this.host.dispose();
    }
    /** Publish a mode once, releasing viewport coordinates while native layout owns the dock. */
    #setDocked(docked) {
        if (docked === this.#docked)
            return;
        this.#docked = docked;
        this.root.dataset.placement = docked ? 'docked' : 'floating';
        this.root.style.removeProperty('left');
        this.root.style.removeProperty('top');
        this.#applied = { x: 0, y: 0 };
        this.changed(docked);
    }
    /** Persist the chosen placement only after a completed pointer or keyboard action. */
    #save() {
        this.store.write({
            docked: this.#requestedDocked,
            floating: this.#desired ? { ...this.#desired } : null,
        });
    }
    /** Accept only the placeholder surface, leaving Run controls outside the drop zone. */
    #nearDock(x, y) {
        const dock = this.host.dockBounds();
        return (!!dock &&
            this.host.canDock() &&
            x >= dock.left &&
            x <= dock.right &&
            y >= dock.top &&
            y <= dock.bottom);
    }
    /** Keep the resting toolbar inside a resized pane while allowing active pointer movement. */
    #hostResized = () => {
        if (!this.#gesture)
            this.#pointerPlaced = false;
        this.#retryDock = true;
        this.#schedule();
    };
    /** Coalesce host resizing and leave animated geometry to its settled destination. */
    #schedule = () => {
        if (this.#frame !== null)
            return;
        const window = this.root.ownerDocument.defaultView;
        if (!window?.requestAnimationFrame)
            return;
        this.#frame = window.requestAnimationFrame(() => {
            this.#frame = null;
            const retryDock = this.#retryDock;
            this.#retryDock = false;
            if (!this.host.attach(this.root, this.#docked) ||
                (retryDock && this.#requestedDocked && !this.#docked)) {
                this.attached();
                return;
            }
            if (this.root.hasAttribute('data-sc-override-moving'))
                this.#place();
            else {
                this.settled();
                const bounds = this.root.getBoundingClientRect();
                this.resize(bounds.width, bounds.height);
            }
        });
    };
    /** Follow viewport coordinates inside the workflow surface without consuming a layout row. */
    #place() {
        if (this.#docked || !this.#parent || this.#size.width <= 0)
            return;
        const surface = this.host.floatingBounds();
        if (!surface)
            return;
        const panel = surface.bounds;
        if (panel.width <= 0 || panel.height <= 0)
            return;
        this.#desired ??= surface.origin;
        const minimumX = panel.left + 4;
        // Native dragging may carry a wide bar past the viewport; keep its recovery controls reachable.
        const visibleWidth = this.#pointerPlaced ? Math.min(88, this.#size.width) : this.#size.width;
        const maximumX = Math.max(minimumX, panel.right - 4 - visibleWidth);
        const maximumY = Math.max(panel.top, panel.bottom - 4 - this.#size.height);
        this.#applied = {
            x: Math.max(minimumX, Math.min(maximumX, this.#desired.x)),
            y: Math.max(panel.top, Math.min(maximumY, this.#desired.y)),
        };
        const left = `${this.#applied.x}px`, top = `${this.#applied.y}px`;
        if (this.root.style.left !== left)
            this.root.style.left = left;
        if (this.root.style.top !== top)
            this.root.style.top = top;
    }
}
