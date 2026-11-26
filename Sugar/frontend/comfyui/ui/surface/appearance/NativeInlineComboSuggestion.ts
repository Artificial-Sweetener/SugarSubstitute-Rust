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
/**
 * Show a searchable choice's suggestion as ghost text in its inline search.
 *
 * Comfy's own combobox keeps filtering, highlighting (Up and Down move the highlight through the
 * narrowed list) and picking (Enter); this owner only shows what the highlight would pick and lets
 * Tab or Right Arrow at the end of the text pick it the same way Enter does.
 */

/** The ghost's two parts: the typed text it repeats invisibly, and the dim text after it. */
export interface SuggestionText {
  readonly typed: string;
  readonly rest: string;
}

/**
 * Decide the ghost for `query` given the highlighted item and the current value.
 *
 * With nothing typed the ghost is the current value. An item that starts with the typed text
 * completes it; any other highlighted item shows its full name after the typed text.
 */
export function suggestionText(
  query: string,
  highlighted: string | null,
  current: string,
): SuggestionText | null {
  if (!query) return current ? { typed: '', rest: current } : null;
  if (!highlighted) return null;
  if (highlighted.toLocaleLowerCase().startsWith(query.toLocaleLowerCase()))
    return highlighted.length > query.length
      ? { typed: query, rest: highlighted.slice(query.length) }
      : null;
  return { typed: query, rest: `  ${highlighted}` };
}

/** Own one inline search's ghost layer and its accept keys, without replacing Comfy's handlers. */
export class NativeInlineComboSuggestion {
  readonly #ghost: HTMLElement;
  readonly #typed: HTMLElement;
  readonly #rest: HTMLElement;
  #shown: SuggestionText | null = null;

  /** Mount the ghost beside `input` and intercept only the accept keys before Comfy sees them. */
  constructor(
    private readonly input: HTMLInputElement,
    private readonly popup: HTMLElement,
    private readonly current: () => string,
  ) {
    const document = input.ownerDocument;
    this.#ghost = document.createElement('div');
    this.#ghost.setAttribute('data-sc-inline-search-ghost', '');
    this.#ghost.setAttribute('aria-hidden', 'true');
    this.#typed = document.createElement('span');
    this.#typed.className = 'sc-inline-search-ghost__typed';
    this.#rest = document.createElement('span');
    this.#rest.className = 'sc-inline-search-ghost__rest';
    this.#ghost.append(this.#typed, this.#rest);
    input.after(this.#ghost);
    input.parentElement?.addEventListener('keydown', this.#accept, true);
    this.update();
  }

  /** Follow the typed text and Comfy's highlight; called once per frame while the search is open. */
  update(): void {
    const highlighted = this.popup.querySelector<HTMLElement>('[role="option"][data-highlighted]');
    const label =
      highlighted?.getAttribute('textvalue') ?? highlighted?.textContent?.trim() ?? null;
    // A scrolled field would misplace the ghost against the text it follows.
    const next =
      this.input.scrollLeft > 0 ? null : suggestionText(this.input.value, label, this.current());
    if (next?.typed === this.#shown?.typed && next?.rest === this.#shown?.rest) return;
    this.#shown = next;
    this.#typed.textContent = next?.typed ?? '';
    this.#rest.textContent = next?.rest ?? '';
    this.#ghost.hidden = next === null;
  }

  /** Mirror the search's geometry so the ghost text lies exactly over the typed text. */
  place(style: CSSStyleDeclaration): void {
    for (const name of ['left', 'top', 'width', 'height', 'font'])
      this.#ghost.style.setProperty(
        `--sc-search-${name}`,
        style.getPropertyValue(`--sc-search-${name}`),
      );
  }

  /** Remove the ghost and the key interception. */
  dispose(): void {
    this.input.parentElement?.removeEventListener('keydown', this.#accept, true);
    this.#ghost.remove();
  }

  /** Pick the highlighted suggestion on Tab, or on Right Arrow with the caret at the end. */
  readonly #accept = (event: KeyboardEvent): void => {
    if (event.target !== this.input || !this.input.value || !this.#shown?.rest) return;
    const atEnd =
      this.input.selectionStart === this.input.value.length &&
      this.input.selectionEnd === this.input.value.length;
    if (event.key !== 'Tab' && !(event.key === 'ArrowRight' && atEnd)) return;
    if (event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    this.input.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'Enter',
        code: 'Enter',
        bubbles: true,
        cancelable: true,
      }),
    );
  };
}
