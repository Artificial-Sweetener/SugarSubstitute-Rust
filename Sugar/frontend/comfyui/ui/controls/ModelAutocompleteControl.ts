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
/** Own token-aware, repeated-entry model autocomplete behavior for every authoring surface. */

const STYLE_ID = 'sugarcubes-model-autocomplete-styles';
let nextControlId = 0;

interface ModelTokenBounds {
  tokenStart: number;
  tokenEnd: number;
  token: string;
}

export interface ModelAutocompleteControlOptions {
  documentRef: Document;
  options: readonly string[];
  value: readonly string[] | string;
  placeholder?: string;
  inputClassName?: string;
  legacyClassNames?: {
    container?: string;
    input?: string;
    listbox?: string;
    suggestion?: string;
  };
}

/** Reuse the original Cube-browser active-token autocomplete without surface-specific policy. */
export class ModelAutocompleteControl {
  readonly element: HTMLElement;
  readonly input: HTMLInputElement;
  readonly listbox: HTMLElement;
  readonly #document: Document;
  readonly #suggestionClassName: string;
  readonly #options: string[];
  readonly #positionSuggestions = (): void => {
    if (this.listbox.hidden || !this.input.isConnected || !this.listbox.isConnected) return;
    const windowRef = this.#document.defaultView;
    if (!windowRef) return;
    const inputBounds = this.input.getBoundingClientRect();
    const viewportMargin = 8;
    const popupGap = 4;
    const desiredHeight = 180;
    const hasRenderedGeometry = inputBounds.width > 0 || inputBounds.height > 0;
    if (
      hasRenderedGeometry &&
      (inputBounds.bottom < viewportMargin ||
        inputBounds.top > windowRef.innerHeight - viewportMargin)
    ) {
      this.#closeSuggestions();
      return;
    }
    const availableBelow = windowRef.innerHeight - inputBounds.bottom - viewportMargin - popupGap;
    const availableAbove = inputBounds.top - viewportMargin - popupGap;
    const openAbove = availableBelow < 96 && availableAbove > availableBelow;
    const availableHeight = openAbove ? availableAbove : availableBelow;
    const maximumWidth = Math.max(0, windowRef.innerWidth - viewportMargin * 2);
    const minimumWidth = Math.min(160, maximumWidth);
    const width = Math.min(Math.max(inputBounds.width, minimumWidth), maximumWidth);
    const left = Math.min(
      Math.max(viewportMargin, inputBounds.left),
      Math.max(viewportMargin, windowRef.innerWidth - viewportMargin - width),
    );
    this.listbox.style.left = `${left}px`;
    this.listbox.style.width = `${width}px`;
    this.listbox.style.maxHeight = `${Math.max(48, Math.min(desiredHeight, availableHeight))}px`;
    if (openAbove) {
      this.listbox.style.top = 'auto';
      this.listbox.style.bottom = `${Math.max(
        viewportMargin,
        windowRef.innerHeight - inputBounds.top + popupGap,
      )}px`;
      return;
    }
    this.listbox.style.top = `${Math.max(
      viewportMargin,
      Math.min(inputBounds.bottom + popupGap, windowRef.innerHeight - viewportMargin),
    )}px`;
    this.listbox.style.bottom = 'auto';
  };
  #suggestions: string[] = [];
  #highlightedIndex = -1;
  #blurTimer: number | null = null;
  #committing = false;

  /** Create one accessible autocomplete for comma-separated model families. */
  constructor(options: ModelAutocompleteControlOptions) {
    this.#document = options.documentRef;
    this.#options = uniqueValues(options.options);
    this.#suggestionClassName = options.legacyClassNames?.suggestion ?? '';
    ensureModelAutocompleteStyles(this.#document);
    const id = `sugarcubes-model-autocomplete-${++nextControlId}`;
    this.element = this.#document.createElement('div');
    this.element.className = joinClassNames(
      'sugarcubes-model-autocomplete',
      options.legacyClassNames?.container,
    );
    this.input = this.#document.createElement('input');
    this.input.type = 'text';
    this.input.className = joinClassNames(
      'sugarcubes-model-autocomplete__input',
      options.inputClassName,
      options.legacyClassNames?.input,
    );
    this.input.placeholder = options.placeholder ?? 'SDXL, Flux .1 D';
    this.input.autocomplete = 'off';
    this.input.value = typeof options.value === 'string' ? options.value : options.value.join(', ');
    this.input.setAttribute('aria-autocomplete', 'list');
    this.input.setAttribute('aria-controls', `${id}-listbox`);
    this.input.setAttribute('aria-expanded', 'false');
    this.input.setAttribute('role', 'combobox');
    this.listbox = this.#document.createElement('div');
    this.listbox.id = `${id}-listbox`;
    this.listbox.className = joinClassNames(
      'sugarcubes-model-autocomplete__suggestions',
      options.legacyClassNames?.listbox,
    );
    this.listbox.setAttribute('role', 'listbox');
    this.listbox.hidden = true;
    this.element.append(this.input);
    (this.#document.body ?? this.#document.documentElement).append(this.listbox);
    this.#document.defaultView?.addEventListener('resize', this.#positionSuggestions);
    this.#document.addEventListener('scroll', this.#positionSuggestions, true);
    this.#bind();
  }

  /** Return normalized entries without changing the author's active text. */
  values(): string[] {
    return parseModelInputValue(this.input.value);
  }

  /** Replace the complete comma-separated value for automatic target-model defaults. */
  setValues(values: readonly string[]): void {
    this.input.value = values.join(', ');
    this.#closeSuggestions();
  }

  /** Release pending delayed work when the owning surface is replaced. */
  dispose(): void {
    if (this.#blurTimer !== null) {
      this.#document.defaultView?.clearTimeout(this.#blurTimer);
      this.#blurTimer = null;
    }
    this.#closeSuggestions();
    this.#document.defaultView?.removeEventListener('resize', this.#positionSuggestions);
    this.#document.removeEventListener('scroll', this.#positionSuggestions, true);
    this.listbox.remove();
  }

  /** Bind the exact interaction policy shared by the browser, modal, and HUD. */
  #bind(): void {
    this.input.addEventListener('input', () => {
      if (!this.#committing) this.#renderSuggestions();
    });
    this.input.addEventListener('focus', () => this.#renderSuggestions());
    this.input.addEventListener('blur', () => {
      const windowRef = this.#document.defaultView;
      if (!windowRef) {
        this.#closeSuggestions();
        return;
      }
      this.#blurTimer = windowRef.setTimeout(() => {
        this.#blurTimer = null;
        if (this.#document.activeElement && this.listbox.contains(this.#document.activeElement)) {
          return;
        }
        this.#closeSuggestions();
      }, 0);
    });
    this.input.addEventListener('keydown', (event) => {
      if (!this.#suggestions.length) return;
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        this.#highlightedIndex = (this.#highlightedIndex + 1) % this.#suggestions.length;
        this.#renderSuggestions(true);
        return;
      }
      if (event.key === 'ArrowUp') {
        event.preventDefault();
        this.#highlightedIndex =
          (this.#highlightedIndex - 1 + this.#suggestions.length) % this.#suggestions.length;
        this.#renderSuggestions(true);
        return;
      }
      if (event.key === 'Enter' || event.key === 'Tab') {
        if (this.#highlightedIndex < 0) return;
        event.preventDefault();
        this.#commitSuggestion(this.#suggestions[this.#highlightedIndex]);
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        this.#closeSuggestions();
      }
    });
  }

  /** Render matching models for only the token containing the current caret. */
  #renderSuggestions(preserveHighlight = false): void {
    const bounds = resolveModelTokenBounds(
      this.input.value,
      this.input.selectionStart,
      this.input.selectionEnd,
    );
    const matches = buildModelSuggestions(bounds.token, this.#options, this.input.value);
    const previousValue =
      preserveHighlight && this.#highlightedIndex >= 0
        ? this.#suggestions[this.#highlightedIndex]
        : '';
    this.#suggestions = matches;
    if (!matches.length) {
      this.#closeSuggestions();
      return;
    }
    if (previousValue) {
      const preservedIndex = matches.indexOf(previousValue);
      this.#highlightedIndex = preservedIndex >= 0 ? preservedIndex : 0;
    } else if (this.#highlightedIndex >= 0) {
      this.#highlightedIndex = Math.min(this.#highlightedIndex, matches.length - 1);
    } else {
      this.#highlightedIndex = 0;
    }
    const children = matches.map((value, index) =>
      this.#createSuggestion(value, index, index === this.#highlightedIndex),
    );
    this.listbox.replaceChildren(...children);
    this.listbox.hidden = false;
    this.input.setAttribute('aria-expanded', 'true');
    this.#positionSuggestions();
  }

  /** Build one literal-text suggestion that commits without stealing input focus. */
  #createSuggestion(value: string, index: number, highlighted: boolean): HTMLButtonElement {
    const button = this.#document.createElement('button');
    button.id = `${this.listbox.id}-option-${index}`;
    button.type = 'button';
    button.className = joinClassNames(
      'sugarcubes-model-autocomplete__suggestion',
      this.#suggestionClassName,
    );
    button.setAttribute('role', 'option');
    button.setAttribute('aria-selected', String(highlighted));
    button.textContent = value;
    if (highlighted) {
      button.classList.add('is-highlighted');
      this.input.setAttribute('aria-activedescendant', button.id);
    }
    button.addEventListener('mousedown', (event) => {
      event.preventDefault();
      this.#commitSuggestion(value);
    });
    return button;
  }

  /** Replace the active token and keep the input ready for another entry. */
  #commitSuggestion(value: string | undefined): void {
    if (!value) return;
    const { tokenStart, tokenEnd } = resolveModelTokenBounds(
      this.input.value,
      this.input.selectionStart,
      this.input.selectionEnd,
    );
    const before = this.input.value.slice(0, tokenStart);
    const after = this.input.value.slice(tokenEnd);
    const normalizedBefore = before.endsWith(',') ? `${before} ` : before;
    const normalizedAfter = after.replace(/^\s*/, '');
    this.input.value = `${normalizedBefore}${value}${normalizedAfter}`;
    const caretPosition = (normalizedBefore + value).length;
    this.input.setSelectionRange(caretPosition, caretPosition);
    this.#closeSuggestions();
    const eventConstructor = this.#document.defaultView?.Event ?? Event;
    this.#committing = true;
    try {
      this.input.dispatchEvent(new eventConstructor('input', { bubbles: true }));
    } finally {
      this.#committing = false;
    }
    this.input.focus();
  }

  /** Clear suggestion state without changing the authored model list. */
  #closeSuggestions(): void {
    this.#suggestions = [];
    this.#highlightedIndex = -1;
    this.listbox.hidden = true;
    this.listbox.replaceChildren();
    this.input.removeAttribute('aria-activedescendant');
    this.input.setAttribute('aria-expanded', 'false');
  }
}

/** Filter matching options while excluding models already selected in other tokens. */
function buildModelSuggestions(
  token: unknown,
  options: readonly string[],
  fullValue: string,
): string[] {
  const normalizedToken = typeof token === 'string' ? token.trim().toLowerCase() : '';
  if (!normalizedToken) return [];
  const existingValues = new Set(
    parseModelInputValue(fullValue)
      .map((entry) => entry.toLowerCase())
      .filter((entry) => entry !== normalizedToken),
  );
  return options.filter((option) => {
    const normalizedOption = option.toLowerCase();
    return normalizedOption.includes(normalizedToken) && !existingValues.has(normalizedOption);
  });
}

/** Parse normalized model entries from a comma-separated authoring value. */
function parseModelInputValue(value: unknown): string[] {
  if (typeof value !== 'string') return [];
  return value
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean);
}

/** Resolve the token surrounding the caret without disturbing adjacent model entries. */
function resolveModelTokenBounds(
  value: unknown,
  selectionStart: number | null,
  selectionEnd: number | null,
): ModelTokenBounds {
  const safeValue = typeof value === 'string' ? value : '';
  const start =
    typeof selectionStart === 'number' && Number.isInteger(selectionStart)
      ? selectionStart
      : safeValue.length;
  const end =
    typeof selectionEnd === 'number' && Number.isInteger(selectionEnd) ? selectionEnd : start;
  const tokenStart = safeValue.lastIndexOf(',', Math.max(0, start - 1)) + 1;
  let tokenEnd = safeValue.indexOf(',', end);
  if (tokenEnd < 0) tokenEnd = safeValue.length;
  const rawToken = safeValue.slice(tokenStart, tokenEnd);
  const leadingWhitespace = rawToken.match(/^\s*/)?.[0].length ?? 0;
  const trailingWhitespace = rawToken.match(/\s*$/)?.[0].length ?? 0;
  return {
    tokenStart: tokenStart + leadingWhitespace,
    tokenEnd: Math.max(tokenStart + leadingWhitespace, tokenEnd - trailingWhitespace),
    token: rawToken.trim(),
  };
}

/** Inject one Comfy-token-based presentation shared by every autocomplete mount. */
function ensureModelAutocompleteStyles(documentRef: Document): void {
  if (documentRef.getElementById(STYLE_ID)) return;
  const style = documentRef.createElement('style');
  style.id = STYLE_ID;
  style.textContent = `
    .sugarcubes-model-autocomplete {
      position: relative;
      display: grid;
      gap: 0.375rem;
      min-width: 0;
    }
    .sugarcubes-model-autocomplete__input {
      box-sizing: border-box;
      width: 100%;
    }
    .sugarcubes-model-autocomplete__suggestions {
      box-sizing: border-box;
      position: fixed;
      z-index: 10020;
      display: grid;
      gap: 0.125rem;
      max-height: 11.25rem;
      overflow-y: auto;
      padding: 0.375rem;
      border: 1px solid var(--border-color, color-mix(in srgb, var(--fg-color, #ddd) 20%, transparent));
      border-radius: var(--p-border-radius-md, 0.5rem);
      background: var(--comfy-menu-bg, var(--p-content-background, #171b20));
      box-shadow: var(--shadow-lg, 0 0.75rem 1.5rem rgb(0 0 0 / 28%));
      color: var(--fg-color, var(--p-text-color, #ddd));
    }
    .sugarcubes-model-autocomplete__suggestions[hidden] {
      display: none;
    }
    .sugarcubes-model-autocomplete__suggestion {
      border: 0;
      border-radius: var(--p-border-radius-sm, 0.25rem);
      background: transparent;
      color: inherit;
      cursor: pointer;
      padding: 0.4rem 0.5rem;
      text-align: left;
      font: inherit;
    }
    .sugarcubes-model-autocomplete__suggestion:hover,
    .sugarcubes-model-autocomplete__suggestion.is-highlighted {
      background: var(--p-content-hover-background, rgb(255 255 255 / 10%));
    }
  `;
  documentRef.head.append(style);
}

/** Join optional class names without emitting whitespace-only tokens. */
function joinClassNames(...values: Array<string | undefined>): string {
  return values
    .map((value) => value?.trim() ?? '')
    .filter(Boolean)
    .join(' ');
}

/** Preserve option order while removing blank and duplicate model names. */
function uniqueValues(values: readonly string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const value of values) {
    const normalized = value.trim();
    const key = normalized.toLowerCase();
    if (!normalized || seen.has(key)) continue;
    seen.add(key);
    result.push(normalized);
  }
  return result;
}
