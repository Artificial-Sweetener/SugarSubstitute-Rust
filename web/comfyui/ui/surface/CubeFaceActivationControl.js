import { createActivationSwitch } from './appearance/ControlFactory.js';
import { readOwnedVisualStyle, subscribeVisualStyle } from './appearance/VisualStyleBoundary.js';
/** Keep a stable header accessory across value updates and visual-style transitions. */
export class CubeFaceActivationControl {
    root;
    #onChange;
    #releaseStyle;
    #card;
    #switch;
    #style;
    /** Create presentation without mutating node state until the user activates the control. */
    constructor(document, card, onChange) {
        this.#card = card;
        this.#onChange = onChange;
        this.root = document.createElement('div');
        this.root.className = 'sugarcubes-cube-face__activation';
        this.root.dataset.cubeCardActivation = card.id;
        this.#style = readOwnedVisualStyle(document);
        this.#switch = createActivationSwitch(document);
        this.#mountSwitch();
        this.root.addEventListener('pointerdown', this.#stopPropagation);
        this.root.addEventListener('click', this.#stopPropagation);
        this.#releaseStyle = subscribeVisualStyle(document, () => this.#refreshStyle());
        this.update(card);
    }
    /** Synchronize external state without replacing the focused control or emitting an edit. */
    update(card) {
        this.#card = card;
        this.root.dataset.enabled = String(card.enabled);
        const actionLabel = `${card.enabled ? 'Disable' : 'Enable'} ${card.label}`;
        this.root.title = actionLabel;
        this.#switch.synchronize(card.enabled, actionLabel, card.enabled ? 'Enabled' : 'Disabled');
    }
    /** Release presentation subscriptions when the native card mount retires. */
    dispose() {
        this.#releaseStyle();
        this.#switch.control.removeEventListener('change', this.#change);
        this.root.removeEventListener('pointerdown', this.#stopPropagation);
        this.root.removeEventListener('click', this.#stopPropagation);
    }
    /** Retain the accessory and node state when the selected provider changes. */
    #refreshStyle() {
        const style = readOwnedVisualStyle(this.root.ownerDocument);
        if (style === this.#style)
            return;
        const control = this.#switch.control;
        const focused = this.root.ownerDocument.activeElement === control ||
            this.root.contains(this.root.ownerDocument.activeElement);
        control.removeEventListener('change', this.#change);
        this.#style = style;
        this.#switch = createActivationSwitch(this.root.ownerDocument);
        this.#mountSwitch();
        this.update(this.#card);
        if (focused)
            this.#switch.control.focus();
    }
    /** Connect exactly one edit callback to the active provider. */
    #mountSwitch() {
        this.#switch.control.addEventListener('change', this.#change);
        this.root.replaceChildren(this.#switch.root);
    }
    /** Forward user edits to the authoritative card-state service exactly once. */
    #change = () => {
        const enabled = this.#switch.control.checked;
        this.update({ ...this.#card, enabled });
        this.#onChange(this.#card.node, enabled);
    };
    /** Keep activation gestures from also invoking titlebar or canvas actions. */
    #stopPropagation = (event) => event.stopPropagation();
}
