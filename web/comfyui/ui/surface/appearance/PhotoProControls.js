import { relocateControl } from '../../core/ControlPlacement.js';
/** Retain ordinary browser input events and a single focus target for the search field. */
class PhotoProFinderInput extends HTMLElement {
    control;
    /** Keep attributes written by declarative views on the actual input. */
    static get observedAttributes() {
        return ['placeholder', 'aria-label', 'disabled'];
    }
    /** Mount the input in light DOM so the shared view can inspect its real value. */
    constructor() {
        super();
        this.control = this.ownerDocument.createElement('input');
        this.control.type = 'search';
        this.control.setAttribute('aria-label', 'Search masonry');
    }
    /** Mount once after construction, as required by the custom-element lifecycle. */
    connectedCallback() {
        if (this.control.parentElement !== this)
            this.append(this.control);
    }
    /** Mirror only supported field attributes across the custom-element boundary. */
    attributeChangedCallback(name, _previous, value) {
        if (value === null)
            this.control.removeAttribute(name);
        else
            this.control.setAttribute(name, value);
    }
    /** Reflect declarative values into the single native input. */
    get value() {
        return this.control.value;
    }
    /** Accept declarative updates without constructing a second input. */
    set value(value) {
        this.control.value = value;
    }
    /** Forward focus to the native input. */
    focus(options) {
        this.control.focus(options);
    }
    /** Select the actual search text. */
    select() {
        this.control.select();
    }
    /** Share the finder owner's explicit focus policy. */
    focusInput(selectAll) {
        this.control.setAttribute('aria-label', this.getAttribute('aria-label') ?? 'Search masonry');
        this.control.focus();
        if (selectAll)
            this.control.select();
    }
}
/** Own only browser tab keyboard and selection mechanics for PhotoPro controls. */
class PhotoProTabGroup extends HTMLElement {
    #activeid = '';
    /** Bind tab gestures to the same change event consumed by shared views. */
    constructor() {
        super();
        this.addEventListener('click', (event) => {
            const tab = event.target instanceof Element ? event.target.closest('[role="tab"]') : null;
            if (tab && this.contains(tab) && !tab.hasAttribute('disabled'))
                this.#select(tab);
        });
        this.addEventListener('keydown', (event) => {
            if (!(event instanceof KeyboardEvent))
                return;
            const tabs = [...this.querySelectorAll('[role="tab"]:not([disabled])')];
            const current = event.target instanceof HTMLElement ? tabs.indexOf(event.target) : -1;
            if (current < 0)
                return;
            const next = event.key === 'ArrowRight'
                ? tabs[(current + 1) % tabs.length]
                : event.key === 'ArrowLeft'
                    ? tabs[(current - 1 + tabs.length) % tabs.length]
                    : event.key === 'Home'
                        ? tabs[0]
                        : event.key === 'End'
                            ? tabs[tabs.length - 1]
                            : event.key === 'Enter' || event.key === ' '
                                ? tabs[current]
                                : null;
            if (!next)
                return;
            event.preventDefault();
            this.#select(next);
            next.focus();
        });
    }
    /** Publish the group role after custom-element construction. */
    connectedCallback() {
        this.setAttribute('role', 'tablist');
    }
    /** Expose the same selected-id contract as the other two tab providers. */
    get activeid() {
        return this.#activeid;
    }
    /** Project host state without emitting another host edit. */
    set activeid(value) {
        this.#activeid = value;
        for (const tab of this.querySelectorAll('[role="tab"]')) {
            const selected = tab.id === value;
            tab.setAttribute('aria-selected', String(selected));
            tab.tabIndex = selected ? 0 : -1;
        }
    }
    /** Emit one user selection while the host remains authoritative for its value. */
    #select(tab) {
        if (tab.id === this.#activeid)
            return;
        this.activeid = tab.id;
        this.dispatchEvent(new Event('change', { bubbles: true }));
    }
}
if (!customElements.get('sc-photopro-search'))
    customElements.define('sc-photopro-search', PhotoProFinderInput);
if (!customElements.get('sc-photopro-tabs'))
    customElements.define('sc-photopro-tabs', PhotoProTabGroup);
/** Keep PhotoPro commands as accessible native buttons. */
export function createPhotoProButton(document) {
    const button = document.createElement('button');
    button.type = 'button';
    return button;
}
/** Adapt the native range value to the existing numeric binding contract. */
export function createPhotoProSlider(document) {
    const input = document.createElement('input');
    input.type = 'range';
    input.setAttribute('aria-label', 'Value');
    return {
        root: input,
        readValue: () => input.valueAsNumber,
        synchronize: (minimum, maximum, step, disabled, value) => {
            input.min = String(minimum);
            input.max = String(maximum);
            input.step = String(step);
            input.disabled = disabled;
            if (value !== null)
                input.valueAsNumber = value;
        },
    };
}
/** Use the browser's checked and disabled state for OpenPhoto's square checkbox. */
export function createPhotoProCheckbox(document) {
    const input = document.createElement('input');
    input.type = 'checkbox';
    return input;
}
/** Share one tab group implementation with native booleans and workflow tabs. */
export function createPhotoProTablist(document) {
    const group = document.createElement('sc-photopro-tabs');
    if (!(group instanceof PhotoProTabGroup))
        throw new TypeError('PhotoPro tabs are not registered.');
    return group;
}
let nextSegment = 0;
/** Present native off/on choices as compact OpenPhoto segments. */
export function createPhotoProSegments(document) {
    const group = createPhotoProTablist(document);
    const off = createPhotoProButton(document);
    const on = createPhotoProButton(document);
    for (const tab of [off, on]) {
        tab.id = `sc-photopro-boolean-${++nextSegment}`;
        tab.setAttribute('role', 'tab');
        tab.tabIndex = -1;
    }
    return { group, segments: [off, on], release: () => group.remove() };
}
/** Provide a stable native search field for masonry. */
export function createPhotoProFinderInput(document) {
    const input = document.createElement('sc-photopro-search');
    if (!(input instanceof PhotoProFinderInput))
        throw new TypeError('PhotoPro search is not registered.');
    return input;
}
/** Keep CodeMirror's document and focus while drawing a compact field frame. */
export function createPhotoProEditorField(document) {
    const root = document.createElement('div');
    root.className = 'sc-photopro-editor-field';
    return {
        root,
        attachEditor(editor) {
            editor.removeAttribute('slot');
            relocateControl(editor, root);
        },
        setReadOnly(readOnly) {
            root.setAttribute('aria-readonly', String(readOnly));
        },
    };
}
/** Expose a light-DOM workflow tab so its title and close action retain identity. */
export function createPhotoProWorkflowTab(document) {
    const tab = document.createElement('div');
    tab.setAttribute('role', 'tab');
    tab.tabIndex = -1;
    return tab;
}
