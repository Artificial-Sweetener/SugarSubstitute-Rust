//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prepare an owned native choice before its existing renderer opens the popup. */
export class NativeChoicePreparation {
    root;
    prepare;
    #pending = null;
    #pointerRequest = null;
    #opening = false;
    #alive = true;
    #replaying = false;
    #cancelled = false;
    /** Keep the native trigger and popup identity while awaiting current external choices. */
    constructor(root, prepare) {
        this.root = root;
        this.prepare = prepare;
        root.addEventListener('pointerdown', this.#pointer, true);
        root.addEventListener('click', this.#open, true);
        root.addEventListener('keydown', this.#open, true);
    }
    /** Retire pending openings before their native field leaves its owning surface. */
    dispose() {
        this.#alive = false;
        this.root.removeEventListener('pointerdown', this.#pointer, true);
        this.root.removeEventListener('click', this.#open, true);
        this.root.removeEventListener('keydown', this.#open, true);
        this.#unwatch();
    }
    /** Resolve only this field's closed, enabled native trigger. */
    #trigger(event) {
        if (this.#replaying || !(event.target instanceof Element))
            return null;
        const trigger = event.target.closest('[role="combobox"]');
        return trigger &&
            this.root.contains(trigger) &&
            trigger.getAttribute('aria-expanded') !== 'true' &&
            trigger.getAttribute('aria-disabled') !== 'true' &&
            !trigger.hasAttribute('disabled')
            ? trigger
            : null;
    }
    /** Begin the read at pointer intent while leaving cancellation and drag gestures closed. */
    #pointer = (event) => {
        const trigger = event.button === 0 ? this.#trigger(event) : null;
        if (!trigger)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        trigger.focus();
        this.#pointerRequest = this.#start(trigger);
    };
    /** Open through the native click path only after current choices have settled. */
    #open = (event) => {
        if (event instanceof KeyboardEvent &&
            (!['Enter', ' ', 'ArrowDown', 'ArrowUp', 'F4'].includes(event.key) ||
                event.ctrlKey ||
                event.metaKey))
            return;
        const trigger = this.#trigger(event);
        if (!trigger)
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
        trigger.focus();
        if (this.#opening)
            return;
        this.#opening = true;
        const preparing = (event instanceof KeyboardEvent ? null : this.#pointerRequest) ?? this.#start(trigger);
        this.#pointerRequest = null;
        void preparing
            .then((ready) => {
            if (!ready ||
                !this.#alive ||
                this.#cancelled ||
                !trigger.isConnected ||
                trigger.ownerDocument.activeElement !== trigger)
                return;
            this.#replaying = true;
            try {
                trigger.click();
            }
            finally {
                this.#replaying = false;
            }
        })
            .finally(() => {
            this.#opening = false;
        });
    };
    /** Coalesce pointer and keyboard intent without applying a late opening after the user leaves. */
    #start(trigger) {
        if (this.#pending)
            return this.#pending;
        this.#cancelled = false;
        trigger.setAttribute('aria-busy', 'true');
        const document = this.root.ownerDocument;
        document.addEventListener('pointerdown', this.#cancel, true);
        document.addEventListener('keydown', this.#cancel, true);
        this.#pending = Promise.resolve()
            .then(this.prepare)
            .catch((error) => {
            console.error('SugarCubes native choice preparation failed', { error });
            return false;
        })
            .finally(() => {
            this.#pending = null;
            trigger.removeAttribute('aria-busy');
            this.#unwatch();
        });
        return this.#pending;
    }
    /** Respect outside input and dismissal while a native request is still in flight. */
    #cancel = (event) => {
        if (event instanceof KeyboardEvent && ['Escape', 'Tab'].includes(event.key))
            this.#cancelled = true;
        else if (event.type === 'pointerdown' &&
            event.target instanceof Node &&
            !this.root.contains(event.target))
            this.#cancelled = true;
    };
    /** Remove temporary dismissal tracking without observing unrelated document mutations. */
    #unwatch() {
        this.root.ownerDocument.removeEventListener('pointerdown', this.#cancel, true);
        this.root.ownerDocument.removeEventListener('keydown', this.#cancel, true);
    }
}
