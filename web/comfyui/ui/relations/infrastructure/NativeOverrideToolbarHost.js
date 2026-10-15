import { NativeRunStripContentVisibility } from './NativeRunStripContentVisibility.js';
import { NativeOverrideToolbarLayout } from './NativeOverrideToolbarLayout.js';
/** Reserve an owned dock slot and measure the width left after native Run and workflow controls. */
export class NativeOverrideToolbarHost {
    document;
    #surfaces = null;
    #slot = null;
    #target = null;
    #dragging = false;
    #emptyContent = new NativeRunStripContentVisibility();
    #layout = new NativeOverrideToolbarLayout();
    /** Resolve only identified native chrome, independently of graph candidate discovery. */
    constructor(document) {
        this.document = document;
    }
    /** Reparent the same toolbar into its dock slot or the workflow overlay. */
    attach(element, docked) {
        this.#surfaces = this.#read();
        const surfaces = this.#surfaces;
        this.#layout.mount(surfaces?.actionbar ?? null);
        if (!surfaces) {
            this.#emptyContent.mount(null);
            element.remove();
            this.#removeSlot();
            this.#removeTarget();
            return false;
        }
        const inline = element.dataset.presentation === 'substitute';
        this.#emptyContent.mount(docked ? surfaces.actionbar : null);
        if (!inline && docked && !this.canDock())
            return false;
        const nativeGrip = surfaces.grip;
        const fontFamily = nativeGrip
            ? this.document.defaultView?.getComputedStyle(nativeGrip).fontFamily
            : undefined;
        if (fontFamily && element.style.getPropertyValue('--sc-native-grip-font-family') !== fontFamily)
            element.style.setProperty('--sc-native-grip-font-family', fontFamily);
        const focused = this.document.activeElement;
        if (docked || inline) {
            if (this.#slot?.parentElement !== surfaces.actionbar) {
                const old = this.#slot;
                this.#slot = this.document.createElement('div');
                this.#slot.className = 'sc-overrides-dock-slot';
                surfaces.actionbar.prepend(this.#slot);
                this.#slot.append(element);
                old?.remove();
            }
            else if (element.parentElement !== this.#slot)
                this.#slot.append(element);
            const width = `${inline ? this.#inlineBudget(surfaces.actionbar) : this.#budget()}px`;
            if (element.style.maxWidth !== width)
                element.style.maxWidth = width;
        }
        else {
            if (element.parentElement !== surfaces.panel ||
                element.previousElementSibling !== surfaces.runRow)
                surfaces.panel.insertBefore(element, surfaces.runRow.nextSibling);
            element.style.maxWidth = `${Math.max(0, surfaces.panel.getBoundingClientRect().width - 8)}px`;
            this.#removeSlot();
        }
        this.#reconcileTarget();
        if (focused instanceof HTMLElement &&
            element.contains(focused) &&
            this.document.activeElement !== focused)
            focused.focus({ preventScroll: true });
        return true;
    }
    /** Bound the floating overlay below workflow tabs and provide its initial position below Run. */
    floatingBounds() {
        const surfaces = this.#surfaces;
        if (!surfaces)
            return null;
        const panel = surfaces.panel.getBoundingClientRect();
        const row = surfaces.runRow.getBoundingClientRect();
        const top = Math.max(panel.top, row.top);
        return {
            bounds: {
                left: panel.left,
                right: panel.right,
                top,
                bottom: panel.bottom,
                width: panel.width,
                height: Math.max(0, panel.bottom - top),
            },
            origin: { x: panel.left + 4, y: row.bottom + 4 },
        };
    }
    /** Expose only the owned placeholder without discovering DOM during pointer movement. */
    dockBounds() {
        return this.#target?.getBoundingClientRect() ?? null;
    }
    /** Decline docking when even the compact controls cannot fit beside native chrome. */
    canDock() {
        return this.#budget() >= 88;
    }
    /** Reserve a dedicated drop destination only for an active moving gesture. */
    dragging(active) {
        if (this.#dragging === active)
            return;
        this.#dragging = active;
        this.#reconcileTarget();
    }
    /** Highlight the dedicated destination without modifying native Run controls. */
    preview(active) {
        this.#target?.toggleAttribute('data-sc-override-drop-target', active);
    }
    /** Notify placement when native Run docking changes the space available in the top strip. */
    subscribeLayout(changed) {
        return this.#layout.subscribe(changed);
    }
    /** Remove owned chrome and release references when the toolbar is disposed. */
    dispose() {
        this.#layout.dispose();
        this.#emptyContent.dispose();
        this.dragging(false);
        this.#removeSlot();
        this.#surfaces = null;
    }
    /** Identify the shared top row and native actionbar through Comfy's mounted surface contract. */
    #read() {
        const canvas = this.document.getElementById('graph-canvas-container');
        const actionbar = canvas?.querySelector('.actionbar-container');
        const panel = actionbar?.closest('.p-splitterpanel');
        if (!actionbar || !panel?.querySelector('.graph-canvas-panel'))
            return null;
        let runRow = actionbar;
        while (runRow.parentElement && runRow.parentElement !== panel)
            runRow = runRow.parentElement;
        if (runRow.parentElement !== panel)
            return null;
        let line = runRow;
        let ancestor = actionbar.parentElement;
        while (ancestor && ancestor !== runRow) {
            if (ancestor.querySelector('button[aria-label="Workflow actions"]')) {
                line = ancestor;
                break;
            }
            ancestor = ancestor.parentElement;
        }
        let runColumn = actionbar;
        while (runColumn !== line && runColumn.parentElement && runColumn.parentElement !== line)
            runColumn = runColumn.parentElement;
        const grip = [...actionbar.querySelectorAll('.actionbar .drag-handle')].find((element) => !element.closest('.sc-overrides-toolbar')) ?? null;
        return { panel, runRow, actionbar, grip, line, runColumn };
    }
    /** Reserve workflow controls and native Run width while excluding feedback in its shared column. */
    #budget() {
        const surfaces = this.#surfaces;
        if (!surfaces)
            return 0;
        const bar = surfaces.actionbar.getBoundingClientRect();
        const slot = this.#slot?.parentElement === surfaces.actionbar
            ? (this.#slot.firstElementChild?.getBoundingClientRect().width ?? 0)
            : 0;
        const targetWidth = this.#target?.parentElement === surfaces.actionbar
            ? Number.parseFloat(this.#target.style.width) || 0
            : 0;
        const nativeWidth = bar.width - slot - (slot ? 8 : 0) - targetWidth - (targetWidth ? 8 : 0);
        const buttons = [...surfaces.line.querySelectorAll('button')].filter((button) => !surfaces.runColumn.contains(button));
        const bounds = buttons
            .map((button) => button.getBoundingClientRect())
            .filter((bound) => bound.width > 0);
        const otherWidth = bounds.length
            ? Math.max(...bounds.map((bound) => bound.right)) -
                Math.min(...bounds.map((bound) => bound.left))
            : 0;
        return Math.max(0, Math.floor(surfaces.line.getBoundingClientRect().width - nativeWidth - otherWidth - 24));
    }
    /** Allocate the inline bar after its retained native view controls without floating-width feedback. */
    #inlineBudget(bar) {
        const children = [...bar.children].filter((child) => child instanceof HTMLElement && child !== this.#slot && child !== this.#target);
        const widths = children
            .map((child) => child.getBoundingClientRect().width)
            .filter((width) => width > 0);
        const gap = Number.parseFloat(this.document.defaultView?.getComputedStyle(bar).columnGap ?? '') || 0;
        return Math.max(0, Math.floor(bar.getBoundingClientRect().width -
            widths.reduce((sum, width) => sum + width, 0) -
            gap * widths.length));
    }
    /** Release an empty owned slot after its toolbar has been moved elsewhere. */
    #removeSlot() {
        this.#slot?.remove();
        this.#slot = null;
    }
    /** Keep one placeholder on the current native surface and exclude its width from dock budgets. */
    #reconcileTarget() {
        const bar = this.#surfaces?.actionbar;
        const budget = this.#budget();
        if (!this.#dragging || !bar || budget < 88) {
            this.#removeTarget();
            return;
        }
        if (this.#target?.parentElement !== bar) {
            this.#removeTarget();
            this.#target = this.document.createElement('div');
            this.#target.className = 'sc-overrides-dock-target';
            this.#target.textContent = 'Dock to Run';
            bar.prepend(this.#target);
        }
        this.#target.style.width = `${Math.min(200, budget)}px`;
    }
    /** Remove the transient target after release, cancellation, or native surface removal. */
    #removeTarget() {
        this.#target?.remove();
        this.#target = null;
    }
}
