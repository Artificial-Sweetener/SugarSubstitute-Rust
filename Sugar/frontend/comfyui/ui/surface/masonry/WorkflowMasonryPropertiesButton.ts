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
/** Gate Comfy's native Properties affordance while masonry owns the workspace. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';

const SURFACES = '.actionbar-container, [data-testid="properties-panel"]';
const ICON = 'i[class~="icon-[lucide--panel-right]"], i[class~="icon-[lucide--panel-left]"]';

interface NativeButtonState {
  disabled: boolean;
  ariaDisabled: string | null;
}

/** Preserve native button state across panel relocation, replacement, and teardown. */
export class WorkflowMasonryPropertiesButton {
  readonly #document: Document;
  readonly #buttons = new Map<HTMLButtonElement, NativeButtonState>();
  #presence: HostSurfaceObserver | null = null;
  #active = false;

  /** Bind native controls by their stable icon rather than translated accessible labels. */
  constructor(documentRef: Document) {
    this.#document = documentRef;
  }

  /** Observe only identified Properties buttons, including the open panel's relocated control. */
  setup(): void {
    this.#presence ??= new HostSurfaceObserver(
      this.#document.body,
      SURFACES,
      () => this.#reconcile(),
      { content: true, attributes: ['disabled', 'aria-disabled'] },
    );
    this.#reconcile();
  }

  /** Apply the native disabled styling and block pointer and keyboard activation. */
  update(active: boolean): void {
    this.#presence?.flushPending();
    this.#active = active;
    this.#reconcile();
  }

  /** Restore host-owned state before releasing native control observation. */
  dispose(): void {
    this.#presence?.disconnect();
    this.#presence = null;
    this.#active = false;
    this.#reconcile();
  }

  /** Reapply only scoped, idempotent changes without triggering graph discovery. */
  #reconcile(): void {
    const buttons = new Set(
      [...this.#document.querySelectorAll<HTMLElement>(SURFACES)].flatMap((surface) =>
        [...surface.querySelectorAll<HTMLButtonElement>('button')].filter((button) =>
          button.querySelector(ICON),
        ),
      ),
    );
    for (const [button, original] of this.#buttons) {
      if (this.#active && buttons.has(button)) continue;
      button.disabled = original.disabled;
      if (original.ariaDisabled === null) button.removeAttribute('aria-disabled');
      else button.setAttribute('aria-disabled', original.ariaDisabled);
      this.#buttons.delete(button);
    }
    if (this.#active)
      for (const button of buttons) {
        if (!this.#buttons.has(button))
          this.#buttons.set(button, {
            disabled: button.disabled,
            ariaDisabled: button.getAttribute('aria-disabled'),
          });
        if (!button.disabled) button.disabled = true;
        if (button.getAttribute('aria-disabled') !== 'true')
          button.setAttribute('aria-disabled', 'true');
      }
    this.#presence?.discardPending();
  }
}
