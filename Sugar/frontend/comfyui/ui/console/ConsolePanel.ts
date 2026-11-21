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
 * Draw the console the way the SugarSubstitute Python application drew its Comfy Console: a
 * titled header with Copy and Clear, over a dark, rounded monospace surface that wraps long lines,
 * colours ANSI output and stays pinned to the newest line. It docks under the card column and is
 * resized from its top edge.
 */
import { createFluentIcon, type FluentIconName } from '../core/FluentIcons.js';
import { parseAnsiLine, plainAnsiLine } from './AnsiText.js';
import { ensureConsoleStyles } from './ConsoleStyles.js';

/** The fewest lines kept before the oldest are dropped, as the Python console's stream did. */
export const CONSOLE_LINE_LIMIT = 5000;
/** The panel's smallest height, in CSS pixels, as the Python console allowed. */
export const CONSOLE_MINIMUM_HEIGHT = 120;
/** The panel's height when first opened, in CSS pixels, as the Python console opened. */
export const CONSOLE_DEFAULT_HEIGHT = 200;
/** How close to the end the view counts as following the output, in CSS pixels. */
const PINNED_SLACK = 24;

/** The console's DOM and its output, without any feed or persistence of its own. */
export class ConsolePanel {
  readonly element: HTMLElement;
  readonly #output: HTMLElement;
  readonly #lines: string[] = [];
  readonly #document: Document;

  /** Build the panel; `copied` receives the text Copy hands to the clipboard. */
  constructor(
    document: Document,
    private readonly copy: (text: string) => void,
    private readonly resized: (height: number) => void,
  ) {
    this.#document = document;
    ensureConsoleStyles(document);
    this.element = document.createElement('section');
    this.element.className = 'sc-console';
    this.element.setAttribute('aria-label', 'Comfy Console');
    const handle = document.createElement('div');
    handle.className = 'sc-console__resize';
    handle.addEventListener('pointerdown', this.#beginResize);
    const header = document.createElement('div');
    header.className = 'sc-console__header';
    const title = document.createElement('span');
    title.className = 'sc-console__title';
    title.textContent = 'Comfy Console';
    header.append(
      title,
      this.#tool('Copy all output', 'copy', () => this.copy(this.text())),
      this.#tool('Clear output', 'broom', () => this.clear()),
    );
    this.#output = document.createElement('pre');
    this.#output.className = 'sc-console__output';
    this.#output.tabIndex = 0;
    this.element.append(handle, header, this.#output);
  }

  /** Append lines, dropping the oldest past the limit and following the end when already there. */
  append(lines: readonly string[]): void {
    if (!lines.length) return;
    const output = this.#output;
    const pinned = output.scrollHeight - output.scrollTop - output.clientHeight <= PINNED_SLACK;
    const fragment = this.#document.createDocumentFragment();
    for (const line of lines) {
      this.#lines.push(line);
      fragment.append(this.#render(line));
    }
    output.append(fragment);
    const excess = this.#lines.length - CONSOLE_LINE_LIMIT;
    if (excess > 0) {
      this.#lines.splice(0, excess);
      for (let index = 0; index < excess; index += 1) output.firstChild?.remove();
    }
    if (pinned) output.scrollTop = output.scrollHeight;
  }

  /** Empty the view; ComfyUI's own output is untouched. */
  clear(): void {
    this.#lines.length = 0;
    this.#output.replaceChildren();
  }

  /** Everything shown, as plain text without colour sequences. */
  text(): string {
    return this.#lines.map(plainAnsiLine).join('\n');
  }

  /** Release the panel's DOM. */
  dispose(): void {
    this.element.remove();
  }

  /** One line as coloured runs of text, never parsed as markup. */
  #render(line: string): HTMLElement {
    const row = this.#document.createElement('div');
    row.className = 'sc-console__line';
    for (const span of parseAnsiLine(line)) {
      if (span.color === null && !span.bold) {
        row.append(span.text);
        continue;
      }
      const run = this.#document.createElement('span');
      run.textContent = span.text;
      if (span.color) run.style.color = span.color;
      if (span.bold) run.style.fontWeight = '700';
      row.append(run);
    }
    if (!row.firstChild) row.append('​');
    return row;
  }

  /** A transparent header tool button. */
  #tool(label: string, icon: FluentIconName, act: () => void): HTMLButtonElement {
    const button = this.#document.createElement('button');
    button.type = 'button';
    button.className = 'sc-console__tool';
    button.title = label;
    button.setAttribute('aria-label', label);
    button.append(createFluentIcon(this.#document, icon));
    button.addEventListener('click', act);
    return button;
  }

  /** Resize from the top edge, keeping at least the minimum and leaving room above. */
  readonly #beginResize = (event: PointerEvent): void => {
    if (event.button !== 0) return;
    event.preventDefault();
    const host = this.element.parentElement;
    if (!host) return;
    const handle = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
    handle?.setPointerCapture(event.pointerId);
    const move = (next: PointerEvent): void => {
      const bounds = host.getBoundingClientRect();
      const height = Math.round(bounds.bottom - next.clientY);
      const maximum = Math.max(CONSOLE_MINIMUM_HEIGHT, bounds.height - CONSOLE_MINIMUM_HEIGHT);
      this.resized(Math.min(maximum, Math.max(CONSOLE_MINIMUM_HEIGHT, height)));
    };
    const end = (): void => {
      handle?.removeEventListener('pointermove', move);
      handle?.removeEventListener('pointerup', end);
      handle?.removeEventListener('pointercancel', end);
      handle?.removeEventListener('lostpointercapture', end);
    };
    handle?.addEventListener('pointermove', move);
    handle?.addEventListener('pointerup', end);
    handle?.addEventListener('pointercancel', end);
    handle?.addEventListener('lostpointercapture', end);
  };
}
