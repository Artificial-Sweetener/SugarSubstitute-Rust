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
/** Safe owned panel rows consume prepared word candidates; they never mutate editable content. */
import type { WordCompletion } from './prompt-completion-model.js';

/** A row retains the immutable candidate it last showed. */
export interface CompletionOptionDOM {
  readonly dom: HTMLElement;
  readonly label: HTMLElement;
  readonly detail: HTMLElement;
  option: WordCompletion;
}

/** Render one literal word row. */
export function createCompletionOption(
  option: WordCompletion,
  id: string,
  index: number,
  count: number,
): CompletionOptionDOM {
  const dom = document.createElement('div');
  dom.id = `${id}-${index}`;
  dom.dataset.index = String(index);
  dom.setAttribute('role', 'option');
  dom.setAttribute('aria-posinset', String(index + 1));
  dom.setAttribute('aria-setsize', String(count));
  dom.setAttribute('aria-label', option.label + (option.detail ? `, ${option.detail}` : ''));
  const label = document.createElement('span'),
    detail = document.createElement('span');
  label.className = 'cm-completionLabel';
  label.textContent = option.label;
  detail.className = 'cm-completionDetail';
  detail.textContent = option.detail;
  dom.className = 'sp-completion-row';
  dom.append(label, detail);
  return { dom, label, detail, option };
}

/**
 * Repaint a row in place for another candidate at the same position.
 *
 * Only text and attributes change, so each keystroke's refreshed list causes no structural DOM change:
 * in Chromium every inserted or removed node restyles ComfyUI's whole document, whose stylesheets carry
 * document-wide `:has()` rules.
 */
export function relabelCompletionRow(
  node: CompletionOptionDOM,
  option: WordCompletion,
  id: string,
  index: number,
  count: number,
): void {
  node.dom.id = `${id}-${index}`;
  node.dom.setAttribute('aria-setsize', String(count));
  node.dom.setAttribute('aria-label', option.label + (option.detail ? `, ${option.detail}` : ''));
  writeText(node.label, option.label);
  writeText(node.detail, option.detail);
  node.option = option;
}

/** Replace a lone text node's data rather than the node itself. */
function writeText(element: HTMLElement, text: string): void {
  const only = element.firstChild;
  if (only?.nodeType === Node.TEXT_NODE && only === element.lastChild) only.nodeValue = text;
  else element.textContent = text;
}
