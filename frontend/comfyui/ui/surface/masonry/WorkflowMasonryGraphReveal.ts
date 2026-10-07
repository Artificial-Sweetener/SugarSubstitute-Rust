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
/** Fade graph-only nodes alongside card travel and preserve their native resting presentation. */
import type { WorkflowMasonrySection } from './WorkflowMasonryProjection.js';

interface GraphNodeOpacity {
  value: string;
  priority: string;
  opacity: string;
}

/** Own suppression only for top-level native nodes absent from the masonry projection. */
export class WorkflowMasonryGraphReveal {
  readonly #nodes = new Map<HTMLElement, GraphNodeOpacity>();
  #hidden = false;

  /** Reconcile known graph presentation membership at topology lifecycles, never popup activity. */
  reconcile(container: HTMLElement, sections: readonly WorkflowMasonrySection[]): void {
    const participating = new Set(
      sections.flatMap((section) =>
        section.cube
          ? [String(section.cube.id)]
          : section.cards.filter((card) => card.visible).map((card) => String(card.node.id)),
      ),
    );
    const root = container.closest('#graph-canvas-container') ?? container;
    const nodes = new Set(
      [...root.querySelectorAll<HTMLElement>('.lg-node[data-node-id]')].filter(
        (element) =>
          !element.parentElement?.closest('.lg-node, .sugarcubes-workflow-masonry') &&
          !participating.has(element.dataset.nodeId ?? ''),
      ),
    );
    for (const [element, saved] of this.#nodes)
      if (!nodes.has(element)) {
        this.#restore(element, saved);
        this.#nodes.delete(element);
      }
    for (const element of nodes) {
      if (this.#nodes.has(element)) continue;
      this.#nodes.set(element, {
        value: element.style.getPropertyValue('opacity'),
        priority: element.style.getPropertyPriority('opacity'),
        opacity: getComputedStyle(element).opacity || '1',
      });
      if (this.#hidden) element.style.setProperty('opacity', '0');
    }
  }

  /** Supply concurrent fades while keeping hidden nodes suppressed before an exit can paint. */
  frames(entering: boolean): Array<{ element: HTMLElement; frames: Keyframe[] }> {
    const requests = [...this.#nodes].map(([element, saved]) => ({
      element,
      frames: [
        { opacity: getComputedStyle(element).opacity || (this.#hidden ? '0' : saved.opacity) },
        { opacity: entering ? '0' : saved.opacity },
      ],
    }));
    this.#hidden = entering;
    // Important native declarations would override the fade; restore their priority at teardown.
    for (const [element, saved] of this.#nodes)
      element.style.setProperty('opacity', entering ? '0' : saved.opacity);
    return requests;
  }

  /** Release every retained node and restore exact native inline opacity on teardown. */
  dispose(): void {
    for (const [element, saved] of this.#nodes) this.#restore(element, saved);
    this.#nodes.clear();
  }

  /** Preserve host priorities and the absence of inline styling. */
  #restore(element: HTMLElement, saved: GraphNodeOpacity): void {
    if (saved.value) element.style.setProperty('opacity', saved.value, saved.priority);
    else element.style.removeProperty('opacity');
  }
}
