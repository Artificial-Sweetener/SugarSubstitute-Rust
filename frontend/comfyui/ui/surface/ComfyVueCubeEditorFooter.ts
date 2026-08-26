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
/** Adapt Comfy's native Nodes 2.0 subgraph footer into a Cube editor action. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';

const EDIT_LABEL = 'Edit Cube';
const DRAG_THRESHOLD = 4;

interface PointerStart {
  id: number;
  x: number;
  y: number;
}

/** Own the label, interaction, and flow measurement of one native footer button. */
export class ComfyVueCubeEditorFooter {
  readonly #root: HTMLElement;
  readonly #node: CubeNode;
  readonly #openEditor: (node: CubeNode) => void;
  #button: HTMLButtonElement | null = null;
  #label: HTMLElement | null = null;
  #originalLabel = '';
  #originalAriaLabel: string | null = null;
  #hadAriaLabel = false;
  #originalTitle: string | null = null;
  #hadTitle = false;
  #pointerStart: PointerStart | null = null;
  #suppressClick = false;

  /** Bind one native node root while preserving Comfy's subgraph navigation command. */
  constructor(root: HTMLElement, node: CubeNode, openEditor: (node: CubeNode) => void) {
    this.#root = root;
    this.#node = node;
    this.#openEditor = openEditor;
    root.addEventListener('pointerdown', this.#onPointerDown, true);
    root.addEventListener('pointerup', this.#onPointerUp, true);
    root.addEventListener('pointercancel', this.#onPointerCancel, true);
    root.addEventListener('click', this.#onClick, true);
    this.reconcile();
  }

  /** Rebind when Vue replaces its native footer subtree. */
  reconcile(): void {
    const button = findOwningFooterButton(this.#root);
    if (button !== this.#button) {
      this.#releaseButton();
      if (button) this.#bindButton(button);
    }
    if (this.#label && this.#label.textContent !== EDIT_LABEL) {
      this.#label.textContent = EDIT_LABEL;
    }
    if (this.#button?.getAttribute('aria-label') !== EDIT_LABEL) {
      this.#button?.setAttribute('aria-label', EDIT_LABEL);
    }
    if (this.#button?.getAttribute('title') !== EDIT_LABEL) {
      this.#button?.setAttribute('title', EDIT_LABEL);
    }
  }

  /** Return the footer's net flex-column contribution, including negative margins. */
  getFlowHeight(): number {
    const wrapper = this.#button?.parentElement;
    const view = wrapper?.ownerDocument.defaultView;
    if (!wrapper || !view) return 0;
    const styles = view.getComputedStyle(wrapper);
    return Math.max(
      0,
      finite(styles.marginTop) + wrapper.offsetHeight + finite(styles.marginBottom),
    );
  }

  /** Restore the native footer before releasing the host seam. */
  dispose(): void {
    this.#root.removeEventListener('pointerdown', this.#onPointerDown, true);
    this.#root.removeEventListener('pointerup', this.#onPointerUp, true);
    this.#root.removeEventListener('pointercancel', this.#onPointerCancel, true);
    this.#root.removeEventListener('click', this.#onClick, true);
    this.#releaseButton();
  }

  /** Capture pointer origin while leaving Comfy's native drag interaction intact. */
  readonly #onPointerDown = (event: PointerEvent): void => {
    if (!findOwningFooterButtonFromTarget(this.#root, event.target)) return;
    this.#pointerStart = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
    };
    this.#suppressClick = false;
  };

  /** Suppress the click Comfy emits after a footer-originated node drag. */
  readonly #onPointerUp = (event: PointerEvent): void => {
    const start = this.#pointerStart;
    this.#pointerStart = null;
    if (!start || start.id !== event.pointerId) return;
    this.#suppressClick =
      Math.hypot(event.clientX - start.x, event.clientY - start.y) > DRAG_THRESHOLD;
  };

  /** Cancel an incomplete pointer gesture without opening the editor. */
  readonly #onPointerCancel = (): void => {
    this.#pointerStart = null;
    this.#suppressClick = true;
  };

  /** Route clicks through SugarCubes' permission-checked editor entry. */
  readonly #onClick = (event: MouseEvent): void => {
    if (!findOwningFooterButtonFromTarget(this.#root, event.target)) return;
    const suppress = this.#suppressClick;
    this.#suppressClick = false;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (!suppress) this.#openEditor(this.#node);
  };

  /** Take ownership of one current native footer button. */
  #bindButton(button: HTMLButtonElement): void {
    this.#button = button;
    this.#label = button.querySelector<HTMLElement>('span.truncate, span');
    this.#originalLabel = this.#label?.textContent ?? '';
    this.#hadAriaLabel = button.hasAttribute('aria-label');
    this.#originalAriaLabel = button.getAttribute('aria-label');
    this.#hadTitle = button.hasAttribute('title');
    this.#originalTitle = button.getAttribute('title');
    if (this.#label) this.#label.textContent = EDIT_LABEL;
    button.setAttribute('aria-label', EDIT_LABEL);
    button.setAttribute('title', EDIT_LABEL);
  }

  /** Release listeners and restore host-owned text and accessibility state. */
  #releaseButton(): void {
    const button = this.#button;
    if (!button) return;
    if (this.#label) this.#label.textContent = this.#originalLabel;
    if (this.#hadAriaLabel && this.#originalAriaLabel !== null) {
      button.setAttribute('aria-label', this.#originalAriaLabel);
    } else {
      button.removeAttribute('aria-label');
    }
    if (this.#hadTitle && this.#originalTitle !== null) {
      button.setAttribute('title', this.#originalTitle);
    } else {
      button.removeAttribute('title');
    }
    this.#button = null;
    this.#label = null;
    this.#pointerStart = null;
    this.#suppressClick = false;
  }
}

/** Resolve a footer event without coupling interaction lifetime to one Vue subtree. */
function findOwningFooterButtonFromTarget(
  root: HTMLElement,
  target: EventTarget | null,
): HTMLButtonElement | null {
  if (!(target instanceof Element)) return null;
  const button = target.closest<HTMLButtonElement>('[data-testid="subgraph-enter-button"]');
  return button && button.closest('.lg-node') === root ? button : null;
}

/** Locate the footer owned by the parent Cube rather than a nested native face card. */
function findOwningFooterButton(root: HTMLElement): HTMLButtonElement | null {
  for (const button of root.querySelectorAll<HTMLButtonElement>(
    '[data-testid="subgraph-enter-button"]',
  )) {
    if (button.closest('.lg-node') === root) return button;
  }
  return null;
}

/** Parse one computed length without allowing invalid geometry to propagate. */
function finite(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}
