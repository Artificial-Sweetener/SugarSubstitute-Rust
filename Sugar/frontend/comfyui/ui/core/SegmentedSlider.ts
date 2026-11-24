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
 * The segmented slider: a row of tabs whose selection is one indicator sliding between them, with
 * an accent mark under the selected tab. The output canvas's source tabs and the settings pages
 * in a plain ComfyUI page both use it.
 */

/** The slider's tab and indicator rules, for one tab list and its indicator. */
export function segmentedSliderCss(list: string, indicator: string): string {
  return `
    ${list} { position:relative; display:flex; align-items:center; gap:0; margin:0; padding:0; overflow-x:auto;
      min-width:0; white-space:nowrap; overscroll-behavior:contain; scrollbar-width:thin; }
    ${list} button { position:relative; background:transparent; border:1px solid transparent; border-radius:5px;
      padding:0 12px; height:30px; min-height:30px; box-shadow:none; font-size:14px; line-height:28px; color:inherit;
      font-family:inherit; cursor:default; }
    ${list} button:hover:not(:disabled) { background:color-mix(in srgb,var(--fg-color,#fff) 3.5%,transparent); }
    ${list} button:active:not(:disabled) { background:color-mix(in srgb,var(--fg-color,#fff) 2.5%,transparent); }
    ${list} button[aria-selected="true"] { box-shadow:none; border-color:transparent; background:transparent; }
    ${indicator} { position:absolute; left:0; top:0; height:30px; box-sizing:border-box; pointer-events:none;
      border:1px solid color-mix(in srgb,var(--fg-color,#fff) 5.5%,transparent); border-radius:5px;
      background:color-mix(in srgb,var(--fg-color,#fff) 6%,transparent); }
    ${indicator}[hidden] { display:none; }
    ${indicator}::after { content:""; position:absolute; bottom:0;
      left:calc(50% - 8px); width:16px; height:3px; border-radius:1.5px; background:var(--p-primary-color,var(--ring,#df258e)); }
    ${indicator}[data-placed] { transition:transform 200ms var(--sc-motion-ease,cubic-bezier(0,0,0,1)),
      width 200ms var(--sc-motion-ease,cubic-bezier(0,0,0,1)); }
    @media (prefers-reduced-motion:reduce) { ${indicator}[data-placed] { transition:none; } }`;
}

/**
 * Cover the selected tab with the indicator. It is placed without motion when it first appears;
 * the stylesheet animates every later move.
 */
export function placeSegmentIndicator(indicator: HTMLElement, tab: HTMLElement | null): void {
  if (!tab) {
    indicator.hidden = true;
    delete indicator.dataset.placed;
    return;
  }
  indicator.hidden = false;
  indicator.style.transform = `translateX(${String(tab.offsetLeft)}px)`;
  indicator.style.width = `${String(tab.offsetWidth)}px`;
  if (!indicator.dataset.placed) {
    void indicator.offsetWidth;
    indicator.dataset.placed = 'true';
  }
}

/** Arrow, Home and End keys over a tab list: the index they move to, or null. */
export function segmentKeyTarget(
  event: KeyboardEvent,
  index: number,
  count: number,
): number | null {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || !count) return null;
  if (event.key === 'Home') return 0;
  if (event.key === 'End') return count - 1;
  if (event.key === 'ArrowRight') return (index + 1) % count;
  if (event.key === 'ArrowLeft') return (index + count - 1) % count;
  return null;
}

/** A segmented slider of labelled tabs, for DOM owners. */
export class SegmentedSlider {
  readonly element: HTMLElement;
  readonly #indicator: HTMLElement;
  readonly #observer: ResizeObserver | null;
  #selected = '';

  constructor(
    document: Document,
    readonly label: string,
    private readonly choose: (id: string) => void,
  ) {
    this.element = document.createElement('nav');
    this.element.className = 'sc-segmented-slider';
    this.element.setAttribute('role', 'tablist');
    this.element.setAttribute('aria-label', label);
    this.#indicator = document.createElement('span');
    this.#indicator.className = 'sc-segmented-slider-indicator';
    this.#indicator.setAttribute('aria-hidden', 'true');
    this.element.append(this.#indicator);
    const Observer = document.defaultView?.ResizeObserver;
    this.#observer = Observer ? new Observer(() => this.#place()) : null;
    this.#observer?.observe(this.element);
  }

  /** Show these tabs, `selected` among them. */
  update(tabs: readonly { id: string; label: string; title?: string }[], selected: string): void {
    this.#selected = selected;
    const buttons = [...this.element.querySelectorAll<HTMLButtonElement>('button')];
    const same =
      buttons.length === tabs.length &&
      buttons.every((button, index) => button.dataset.tab === tabs[index]?.id);
    if (!same) {
      for (const button of buttons) button.remove();
      tabs.forEach((tab, index) => {
        const button = this.element.ownerDocument.createElement('button');
        button.type = 'button';
        button.setAttribute('role', 'tab');
        button.dataset.tab = tab.id;
        button.textContent = tab.label;
        if (tab.title) button.title = tab.title;
        button.addEventListener('click', () => this.choose(tab.id));
        button.addEventListener('keydown', (event) => {
          const next = segmentKeyTarget(event, index, tabs.length);
          if (next === null) return;
          event.preventDefault();
          event.stopPropagation();
          const target = tabs[next];
          if (!target) return;
          this.element.querySelector<HTMLButtonElement>(`button[data-tab="${target.id}"]`)?.focus();
          this.choose(target.id);
        });
        this.element.append(button);
      });
    }
    for (const button of this.element.querySelectorAll<HTMLButtonElement>('button')) {
      const on = button.dataset.tab === selected;
      button.setAttribute('aria-selected', String(on));
      button.tabIndex = on ? 0 : -1;
    }
    this.#place();
  }

  dispose(): void {
    this.#observer?.disconnect();
  }

  #place(): void {
    placeSegmentIndicator(
      this.#indicator,
      [...this.element.querySelectorAll<HTMLElement>('button')].find(
        (button) => button.dataset.tab === this.#selected,
      ) ?? null,
    );
  }
}
