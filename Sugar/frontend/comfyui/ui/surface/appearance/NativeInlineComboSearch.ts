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
/** Place Comfy's own live search input over its field while preserving native filtering and navigation. */
interface SearchBinding {
  trigger: HTMLElement;
  popup: HTMLElement;
  input: HTMLInputElement;
  shell: HTMLElement;
  label: string | null;
}

const geometryProperties = ['left', 'top', 'width', 'height', 'font'] as const;

/** Own the reversible geometry of one native searchable combobox, without duplicating its state. */
export class NativeInlineComboSearch {
  #binding: SearchBinding | null = null;
  #frame: number | null = null;
  readonly #document: Document;

  /** Follow scrolling, graph transforms, and popup placement only while a search is open. */
  constructor(documentRef: Document) {
    this.#document = documentRef;
  }

  /** Project only the positively identified native default-combobox input into its original field. */
  present(trigger: HTMLElement, popup: HTMLElement): void {
    if (this.#binding?.input.isConnected && this.#binding.popup === popup) return;
    this.clear();
    if (trigger.dataset.testid !== 'widget-select-default-trigger') return;
    const input = popup.querySelector('[data-testid="widget-select-default-search-input"]');
    if (!(input instanceof HTMLInputElement) || !input.parentElement) return;
    const shell = input.parentElement;
    this.#binding = { trigger, popup, input, shell, label: input.getAttribute('aria-label') };
    input.setAttribute('data-sc-inline-search', '');
    shell.setAttribute('data-sc-inline-search-shell', '');
    input.setAttribute('aria-label', trigger.getAttribute('aria-label') ?? 'Search');
    this.#layout();
    this.#schedule();
  }

  /** Release styles before theme suspension or after the host closes its popup. */
  clear(): void {
    const binding = this.#binding;
    this.#binding = null;
    if (this.#frame !== null) this.#document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    if (!binding) return;
    binding.input.removeAttribute('data-sc-inline-search');
    binding.shell.removeAttribute('data-sc-inline-search-shell');
    if (binding.label === null) binding.input.removeAttribute('aria-label');
    else binding.input.setAttribute('aria-label', binding.label);
    for (const property of geometryProperties)
      binding.input.style.removeProperty(`--sc-search-${property}`);
  }

  /** Restore only the search belonging to a retired native popup relationship. */
  release(popup: HTMLElement): void {
    if (this.#binding?.popup === popup) this.clear();
  }

  /** Stop tracking a detached search input without retaining its host component tree. */
  reconcile(): void {
    if (this.#binding && (!this.#binding.input.isConnected || !this.#binding.trigger.isConnected))
      this.clear();
  }

  /** Coalesce geometry reads and follow Comfy's compositor-driven graph pan and zoom. */
  readonly #schedule = (): void => {
    if (!this.#binding || this.#frame !== null) return;
    this.#frame =
      this.#document.defaultView?.requestAnimationFrame(() => {
        this.#frame = null;
        this.reconcile();
        if (!this.#binding) return;
        this.#layout();
        this.#schedule();
      }) ?? null;
  };

  /** Keep the native search field precisely within the trigger's screen bounds. */
  #layout(): void {
    const binding = this.#binding;
    if (!binding) return;
    const trigger = binding.trigger.getBoundingClientRect();
    const popup = binding.popup.getBoundingClientRect();
    const field = binding.trigger.closest<HTMLElement>('[data-capture-wheel]') ?? binding.trigger;
    const fieldRectangle = field.getBoundingClientRect();
    const scale = fieldRectangle.height / Math.max(1, field.offsetHeight);
    const popupStyle = this.#document.defaultView?.getComputedStyle(binding.popup);
    const font = Number.parseFloat(
      this.#document.defaultView?.getComputedStyle(binding.trigger).fontSize || '12',
    );
    const values = {
      left:
        fieldRectangle.left - popup.left - Number.parseFloat(popupStyle?.borderLeftWidth || '0'),
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
