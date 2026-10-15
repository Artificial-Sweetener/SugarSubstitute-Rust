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
/** Remove empty legacy Run-strip space while retaining live native extension controls. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';

/** Observe only the legacy content boundary and reversibly collapse its empty placeholder. */
export class NativeRunStripContentVisibility {
  #element: HTMLElement | null = null;
  #observer: HostSurfaceObserver | null = null;

  /** Bind a new native surface; its scoped observer owns subsequent content refreshes. */
  mount(actionbar: HTMLElement | null): void {
    const element =
      actionbar?.querySelector<HTMLElement>(':scope > [data-testid="legacy-topbar-container"]') ??
      null;
    if (element === this.#element) return;
    this.dispose();
    this.#element = element;
    if (element)
      this.#observer = new HostSurfaceObserver(
        element,
        '[data-testid="legacy-topbar-container"]',
        () => this.#reconcile(),
        { content: true, characterData: true, attributes: ['class', 'style', 'hidden'] },
      );
    this.#reconcile();
  }

  /** Release observation and restore the native container's own visibility policy. */
  dispose(): void {
    this.#observer?.disconnect();
    this.#observer = null;
    this.#element?.removeAttribute('data-sc-empty-run-content');
    this.#element = null;
  }

  /** Ignore our placeholder suppression so newly visible controls can restore the container. */
  #reconcile(): void {
    const element = this.#element;
    if (!element) return;
    const empty = !this.#hasVisibleChildren(element);
    if (element.hasAttribute('data-sc-empty-run-content') !== empty)
      element.toggleAttribute('data-sc-empty-run-content', empty);
  }

  /** Treat hidden controls as empty while keeping text, graphics, and native input surfaces. */
  #hasVisibleContent(element: Element): boolean {
    const style = element.ownerDocument.defaultView?.getComputedStyle(element);
    if (!style) return true;
    if (
      element.hasAttribute('hidden') ||
      style.display === 'none' ||
      style.visibility === 'hidden' ||
      style.visibility === 'collapse'
    )
      return false;
    if (
      element.matches(
        'button,input,select,textarea,img,svg,canvas,video,audio,iframe,[contenteditable="true"]',
      ) ||
      element.localName.includes('-')
    )
      return true;
    return this.#hasVisibleChildren(element);
  }

  /** Inspect actual content without treating empty layout wrappers as controls. */
  #hasVisibleChildren(element: Element): boolean {
    return [...element.childNodes].some((node) =>
      node instanceof Element
        ? this.#hasVisibleContent(node)
        : node.nodeType === Node.TEXT_NODE && Boolean(node.textContent?.trim()),
    );
  }
}
