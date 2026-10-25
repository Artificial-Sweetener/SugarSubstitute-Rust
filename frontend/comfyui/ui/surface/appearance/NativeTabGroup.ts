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
/** Own native tab keyboard, selection and focus semantics independently of visual paint. */
/** Own only browser tab keyboard and selection mechanics for native visual-style providers. */
export class NativeTabGroup extends HTMLElement {
  #activeid = '';

  /** Bind tab gestures to the same change event consumed by shared views. */
  constructor() {
    super();
    this.addEventListener('click', (event) => {
      const tab =
        event.target instanceof Element ? event.target.closest<HTMLElement>('[role="tab"]') : null;
      if (tab && this.contains(tab) && !tab.hasAttribute('disabled')) this.#select(tab);
    });
    this.addEventListener('keydown', (event) => {
      if (!(event instanceof KeyboardEvent)) return;
      const tabs = [...this.querySelectorAll<HTMLElement>('[role="tab"]:not([disabled])')];
      const current = event.target instanceof HTMLElement ? tabs.indexOf(event.target) : -1;
      if (current < 0) return;
      const next =
        event.key === 'ArrowRight'
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
      if (!next) return;
      event.preventDefault();
      this.#select(next);
      next.focus();
    });
  }

  /** Publish the group role after custom-element construction. */
  connectedCallback(): void {
    this.setAttribute('role', 'tablist');
  }

  /** Expose the same selected-id contract as the other two tab providers. */
  get activeid(): string {
    return this.#activeid;
  }

  /** Project host state without emitting another host edit. */
  set activeid(value: string) {
    this.#activeid = value;
    for (const tab of this.querySelectorAll<HTMLElement>('[role="tab"]')) {
      const selected = tab.id === value;
      tab.setAttribute('aria-selected', String(selected));
      tab.tabIndex = selected ? 0 : -1;
    }
  }

  /** Emit one user selection while the host remains authoritative for its value. */
  #select(tab: HTMLElement): void {
    if (tab.id === this.#activeid) return;
    this.activeid = tab.id;
    this.dispatchEvent(new Event('change', { bubbles: true }));
  }
}
