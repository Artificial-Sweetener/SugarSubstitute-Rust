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
/** Decorate Comfy's own choice list with shared-domain restrictions without replacing its renderer. */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
/** Follow the exact owned combobox portal and prevent disabled native options from committing. */
export class NativeChoiceRestrictions {
    root;
    #observer;
    #portal = null;
    #boundList = null;
    #choices = [];
    #locks = new Set();
    /** Observe native list mounts and retained search results while keeping menus under Comfy ownership. */
    constructor(root) {
        this.root = root;
        this.#observer = new MutationObserver(() => this.#followList());
        this.#observer.observe(root, {
            childList: true,
            subtree: true,
            attributes: true,
            attributeFilter: ['aria-controls', 'aria-expanded'],
        });
        root.addEventListener('keydown', this.#guard, true);
    }
    /** Recalculate locks when a participant or a native domain changes. */
    update(choices) {
        this.#choices = choices;
        this.#followList();
    }
    /** Release portal decorations and input guards before unmounting the owner widget. */
    dispose() {
        this.#observer.disconnect();
        this.#portal?.disconnect();
        this.#bindList(null);
        for (const option of this.#locks)
            this.#restore(option);
        this.root.removeEventListener('keydown', this.#guard, true);
    }
    /** Observe the native portal only while this restricted trigger is open. */
    #followList() {
        const restricted = this.#choices.some((choice) => choice.blockers.length > 0);
        const trigger = this.root.querySelector('[role="combobox"]');
        const open = restricted && trigger?.getAttribute('aria-expanded') === 'true';
        if (open && !this.#portal)
            this.#portal = new HostSurfaceObserver(this.root.ownerDocument.body, '[role="listbox"]', () => {
                this.#bindList(this.#list());
                this.#decorate();
            }, { content: true });
        if (!open) {
            this.#portal?.disconnect();
            this.#portal = null;
        }
        this.#bindList(restricted ? this.#list() : null);
        this.#decorate();
    }
    /** Keep input guards on the trigger's own native list rather than on every document event. */
    #bindList(list) {
        if (list === this.#boundList)
            return;
        for (const event of ['pointerdown', 'click', 'keydown'])
            this.#boundList?.removeEventListener(event, this.#guard, true);
        this.#boundList = list;
        for (const event of ['pointerdown', 'click', 'keydown'])
            list?.addEventListener(event, this.#guard, true);
    }
    /** Resolve the list identified by the native trigger rather than scanning unrelated menus. */
    #list() {
        const id = this.root.querySelector('[role="combobox"]')?.getAttribute('aria-controls');
        return id ? this.root.ownerDocument.getElementById(id) : null;
    }
    /** Keep disabled choices searchable and visible with their blocking participant explanation. */
    #decorate() {
        const list = this.#list();
        for (const option of this.#locks)
            if (!option.isConnected || !list?.contains(option))
                this.#restore(option);
        for (const option of list?.querySelectorAll('[role="option"]') ?? []) {
            const label = option.querySelector('span')?.textContent ?? option.textContent ?? '';
            const choice = this.#choices.find((item) => item.value === label.trim());
            if (!choice?.blockers.length) {
                if (this.#locks.has(option))
                    this.#restore(option);
                continue;
            }
            const reason = `Unavailable for ${choice.blockers.join(', ')}`;
            option.setAttribute('aria-disabled', 'true');
            option.setAttribute('aria-description', reason);
            option.title = reason;
            option.dataset.scChoiceLocked = '';
            if (!option.querySelector('[data-sc-choice-lock]')) {
                const lock = option.ownerDocument.createElement('span');
                lock.dataset.scChoiceLock = '';
                lock.className = 'icon-[lucide--lock-keyhole] size-3.5';
                lock.setAttribute('aria-hidden', 'true');
                option.append(lock);
            }
            this.#locks.add(option);
        }
    }
    /** Reject pointer selection and Enter on a highlighted locked option before Reka handles them. */
    #guard = (event) => {
        const list = this.#list();
        if (!list || !(event.target instanceof Element))
            return;
        const option = event.target.closest('[data-sc-choice-locked]') ??
            (event instanceof KeyboardEvent &&
                (event.key === 'Enter' || event.key === ' ') &&
                (list.contains(event.target) || this.root.contains(event.target))
                ? list.querySelector('[data-highlighted][data-sc-choice-locked]')
                : null);
        if (!option || !list.contains(option))
            return;
        if (event instanceof KeyboardEvent && event.key !== 'Enter' && event.key !== ' ')
            return;
        event.preventDefault();
        event.stopImmediatePropagation();
    };
    /** Restore only restriction annotations owned by this adapter. */
    #restore(option) {
        option.removeAttribute('aria-disabled');
        option.removeAttribute('aria-description');
        option.removeAttribute('title');
        delete option.dataset.scChoiceLocked;
        option.querySelector('[data-sc-choice-lock]')?.remove();
        this.#locks.delete(option);
    }
}
