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
/** Safe owned tooltip nodes consume prepared options and decoded rasters; they never acquire media or mutate editable content. */
import type { PreparedCompletion } from './prompt-completion-model.js';
import type { LoraMediaSnapshot } from './lora-media-state.js';
import {
  pendingLoraMedia,
  unavailableLoraMedia,
  type PreparedLoraMedia,
} from './lora-media-contracts.js';

/** An option node retains immutable candidate provenance and the actual raster it last painted. */
export interface CompletionOptionDOM {
  readonly dom: HTMLElement;
  readonly label: HTMLElement;
  readonly detail: HTMLElement;
  option: PreparedCompletion;
  readonly backing: HTMLElement | null;
  media: PreparedLoraMedia | null;
}

/** Render literal host labels, with one standard structure for thumbnail and fallback model tiles. */
export function createCompletionOption(
  option: PreparedCompletion,
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
  let backing: HTMLElement | null = null;
  if (option.kind === 'lora') {
    detail.textContent = option.item.subtitle ?? '';
    dom.dataset.subtitle = String(!!option.item.subtitle);
    dom.className = 'sp-completion-tile';
    const titleText = document.createElement('span');
    titleText.className = 'sp-completion-title-text';
    titleText.dir = 'auto';
    titleText.textContent = option.label;
    label.replaceChildren(titleText);
    dom.title = `${option.label}${option.item.subtitle ? ` - ${option.item.subtitle}` : ''}\n${option.item.model.backendValue}`;
    backing = document.createElement('div');
    backing.className = 'sp-completion-backing';
    backing.setAttribute('aria-hidden', 'true');
    const initial = document.createElement('span');
    initial.className = 'sp-completion-initial';
    initial.textContent = 'No image';
    initial.setAttribute('aria-hidden', 'true');
    dom.append(backing, initial);
  } else dom.className = 'sp-completion-row';
  dom.append(label, detail);
  return { dom, label, detail, option, backing, media: null };
}

/**
 * Repaint a plain row in place for another candidate at the same position.
 *
 * Only text and attributes change, so each keystroke's refreshed list causes no structural DOM change:
 * in Chromium every inserted or removed node restyles ComfyUI's whole document, whose stylesheets carry
 * document-wide `:has()` rules. Model tiles own raster backing and are rebuilt instead.
 */
export function relabelCompletionRow(
  node: CompletionOptionDOM,
  option: PreparedCompletion,
  id: string,
  index: number,
  count: number,
): boolean {
  if (node.option.kind === 'lora' || option.kind === 'lora') return false;
  node.dom.id = `${id}-${index}`;
  node.dom.setAttribute('aria-setsize', String(count));
  node.dom.setAttribute('aria-label', option.label + (option.detail ? `, ${option.detail}` : ''));
  writeText(node.label, option.label);
  writeText(node.detail, option.detail);
  node.option = option;
  return true;
}

/** Replace a lone text node's data rather than the node itself. */
function writeText(element: HTMLElement, text: string): void {
  const only = element.firstChild;
  if (only?.nodeType === Node.TEXT_NODE && only === element.lastChild) only.nodeValue = text;
  else element.textContent = text;
}

/** Actual descriptor/raster identity prevents repaint on unrelated source selection or geometry changes. */
export function paintCompletionMedia(node: CompletionOptionDOM, media: LoraMediaSnapshot): void {
  if (node.option.kind !== 'lora' || !node.backing) return;
  const thumbnail = node.option.item.thumbnail;
  const prepared =
    thumbnail.status === 'ready'
      ? (media.get(thumbnail) ?? pendingLoraMedia)
      : unavailableLoraMedia;
  if (node.media === prepared) return;
  node.media = prepared;
  node.dom.dataset.media = prepared.status;
  node.dom.setAttribute(
    'aria-label',
    `${node.option.label}, ${node.option.item.model.backendValue}${node.option.item.subtitle ? `, ${node.option.item.subtitle}` : ''}, thumbnail ${prepared.status}`,
  );
  node.backing.replaceChildren();
  if (prepared.status === 'ready') {
    const canvas = document.createElement('canvas'),
      raster = prepared.raster;
    canvas.width = raster.width;
    canvas.height = raster.height;
    canvas.setAttribute('aria-hidden', 'true');
    raster.paint(canvas);
    node.backing.append(canvas);
  }
}
