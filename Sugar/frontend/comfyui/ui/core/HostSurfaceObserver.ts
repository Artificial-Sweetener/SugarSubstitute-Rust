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
/** Translate native DOM lifecycle into identified surface changes before notifying consumers. */
export interface HostSurfaceChange {
  readonly element: HTMLElement;
  readonly kind: 'mounted' | 'removed' | 'content';
  readonly relocated?: boolean;
}

export interface HostSurfaceObservation {
  readonly content?: boolean;
  readonly characterData?: boolean;
  readonly attributes?: readonly string[];
  readonly exclude?: string;
}

/** Filter host mutations at their source without promoting a portal's parent to a discovery root. */
export class HostSurfaceObserver {
  readonly #observer: MutationObserver;
  readonly #process: (records: readonly MutationRecord[]) => void;

  /** Observe named host boundaries and report only actual mounts or explicitly requested local changes. */
  constructor(
    root: Node,
    selector: string,
    changed: (changes: readonly HostSurfaceChange[]) => void,
    options: HostSurfaceObservation = {},
  ) {
    this.#process = (records) => {
      const excluded = new WeakMap<Element, boolean>();
      /** Treat explicitly owned rendering trees as opaque and share ancestry checks within each delivery. */
      const isExcluded = (element: Element): boolean => {
        const path: Element[] = [];
        let current: Element | null = element;
        let result = false;
        while (current) {
          const cached = excluded.get(current);
          if (cached !== undefined) {
            result = cached;
            break;
          }
          path.push(current);
          if (
            current.hasAttribute('data-sc-host-opaque') ||
            (options.exclude && current.matches(options.exclude))
          ) {
            result = true;
            break;
          }
          current = current.parentElement;
        }
        for (const ancestor of path) excluded.set(ancestor, result);
        return result;
      };
      const changes = new Map<HTMLElement, HostSurfaceChange['kind']>();
      const mounted = new Set<HTMLElement>();
      const removed = new Set<HTMLElement>();
      const content = new Set<HTMLElement>();
      const collect = (node: Node, kind: 'mounted' | 'removed'): void => {
        if (!(node instanceof HTMLElement) || isExcluded(node)) return;
        if (node.matches(selector)) {
          changes.set(node, kind);
          (kind === 'mounted' ? mounted : removed).add(node);
        }
        if (!node.childElementCount) return;
        for (const element of node.querySelectorAll<HTMLElement>(selector))
          if (!isExcluded(element)) {
            changes.set(element, kind);
            (kind === 'mounted' ? mounted : removed).add(element);
          }
      };
      for (const record of records) {
        const target =
          record.target instanceof Element ? record.target : record.target.parentElement;
        if (target && isExcluded(target)) continue;
        const surface =
          options.content || record.type === 'attributes'
            ? target?.closest<HTMLElement>(selector)
            : null;
        if (surface) {
          changes.set(surface, 'content');
          content.add(surface);
        }
        for (const node of record.addedNodes) collect(node, 'mounted');
        for (const node of record.removedNodes) collect(node, 'removed');
      }
      if (changes.size)
        changed(
          [...changes].map(([element, kind]) => {
            if (mounted.has(element) && removed.has(element) && element.isConnected)
              return {
                element,
                kind: content.has(element) ? 'content' : 'mounted',
                relocated: true,
              };
            return { element, kind };
          }),
        );
    };
    this.#observer = new MutationObserver(this.#process);
    this.#observer.observe(root, {
      childList: true,
      subtree: true,
      characterData: options.characterData ?? false,
      ...(options.attributes?.length
        ? { attributes: true, attributeFilter: [...options.attributes] }
        : {}),
    });
  }

  /** Deliver queued host mounts before a synchronous presentation reconciliation. */
  flushPending(): void {
    this.#process(this.#observer.takeRecords());
  }

  /** Discard mutations produced synchronously by the consumer's own presentation update. */
  discardPending(): void {
    this.#observer.takeRecords();
  }

  /** Release native observation with the host adapter's lifetime. */
  disconnect(): void {
    this.#observer.disconnect();
  }
}
