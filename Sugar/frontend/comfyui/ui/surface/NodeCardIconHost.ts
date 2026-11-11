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
/** Own decorative node-title icons within one projected native card header. */
import type { ComfyNode } from '../types/graph.js';
import { resolveNodeCardIcon, type NodeCardIcon } from './NodeCardIconPolicy.js';

const iconFiles: Readonly<Record<NodeCardIcon, string>> = {
  application: 'app-generic.svg',
  edit: 'edit.svg',
  eraser: 'eraser.svg',
  folder: 'folder.svg',
  model: 'brain-circuit.svg',
  palette: 'color.svg',
  photo: 'image.svg',
};

/** Reattach a decorative glyph when Vue replaces its title, preserving native content/events. */
export class NodeCardIconHost {
  readonly #cell: HTMLElement;
  readonly #node: ComfyNode;
  readonly #icon: HTMLElement;
  readonly #observer: MutationObserver;
  #title: HTMLElement | null = null;
  #disposed = false;

  /** Bind only to an explicitly marked projection cell, independent of the appearance theme. */
  constructor(cell: HTMLElement, node: ComfyNode) {
    this.#cell = cell;
    this.#node = node;
    this.#icon = cell.ownerDocument.createElement('span');
    this.#icon.className = 'sugarcubes-workflow-masonry__node-icon';
    this.#icon.setAttribute('aria-hidden', 'true');
    this.#observer = new MutationObserver(() => this.refresh());
    this.#observer.observe(cell, { childList: true, subtree: true });
    this.refresh();
  }

  /** Resolve the current exact header and update only SugarCubes-owned icon state. */
  refresh(): void {
    if (this.#disposed) return;
    const icon =
      this.#cell.hasAttribute('data-cube-node-id') || this.#cell.hasAttribute('data-masonry-card')
        ? resolveNodeCardIcon(this.#node)
        : null;
    const title =
      [...this.#cell.querySelectorAll<HTMLElement>('[data-testid="node-title"]')].find(
        (candidate) =>
          candidate.closest('[data-cube-node-id], [data-masonry-card]') === this.#cell &&
          candidate.closest('[data-testid^="node-header-"]')?.getAttribute('data-testid') ===
            `node-header-${String(this.#node.id ?? '')}`,
      ) ?? null;
    if (this.#title !== title || !icon) this.#releaseTitle();
    if (!title || !icon) return;
    this.#title = title;
    title.dataset.masonryTitleIcon = '';
    if (this.#icon.dataset.masonryNodeIcon !== icon) {
      this.#icon.dataset.masonryNodeIcon = icon;
      const url = new URL(`../../../icons/node/${iconFiles[icon]}`, import.meta.url);
      this.#icon.style.setProperty('--sc-node-icon', `url("${url.href}")`);
    }
    if (this.#icon.parentElement !== title) title.prepend(this.#icon);
  }

  /** Stop observing before the native card is released, including detached-title markers. */
  dispose(): void {
    this.#disposed = true;
    this.#observer.disconnect();
    this.#releaseTitle();
  }

  /** Remove only our decorative element and its title-layout marker. */
  #releaseTitle(): void {
    if (this.#title) delete this.#title.dataset.masonryTitleIcon;
    this.#icon.remove();
    this.#title = null;
  }
}
