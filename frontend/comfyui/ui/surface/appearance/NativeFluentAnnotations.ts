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
/** Reconcile exact native appearance annotations without tearing down unchanged styles. */
type ElementAnnotations = Map<HTMLElement, Map<string, string>>;

/** Own removable appearance properties independently of native component discovery. */
export class NativeFluentAnnotations {
  readonly #roots = new Map<HTMLElement, ElementAnnotations>();

  /** Begin one scoped reconciliation while releasing detached native controls. */
  begin(root: HTMLElement): NativeFluentAnnotationPlan {
    this.prune();
    const previous = this.#roots.get(root) ?? new Map();
    return new NativeFluentAnnotationPlan(previous, (next) => this.#roots.set(root, next));
  }

  /** Release retired presentation roots even when the batch contains no surviving native cards. */
  prune(): void {
    for (const [element, annotations] of this.#roots)
      if (!element.isConnected) {
        clearRemoved(annotations, new Map());
        this.#roots.delete(element);
      }
  }

  /** Release exactly the properties still owned by the active appearance. */
  clear(): void {
    for (const annotations of this.#roots.values()) clearRemoved(annotations, new Map());
    this.#roots.clear();
  }
}

/** Collect one native boundary's ownership and write only materially changed attributes or styles. */
export class NativeFluentAnnotationPlan {
  readonly #next: ElementAnnotations = new Map();
  readonly #previous: ElementAnnotations;
  readonly #commit: (next: ElementAnnotations) => void;

  /** Retain prior ownership until native discovery has identified the complete next boundary. */
  constructor(previous: ElementAnnotations, commit: (next: ElementAnnotations) => void) {
    this.#previous = previous;
    this.#commit = commit;
  }

  /** Mark one native boundary without generating redundant attribute mutations. */
  attribute(element: HTMLElement, name: string, value = ''): void {
    this.#record(element, name, value);
    if (element.getAttribute(name) !== value) element.setAttribute(name, value);
  }

  /** Retain native row placement while avoiding repeated style invalidation. */
  style(element: HTMLElement, name: string, value: string): void {
    this.#record(element, name, value);
    if (element.style.getPropertyValue(name) !== value) element.style.setProperty(name, value);
  }

  /** Remove disappeared ownership after discovery, retaining unchanged native control identity. */
  commit(): void {
    clearRemoved(this.#previous, this.#next);
    this.#commit(this.#next);
  }

  /** Record the exact property so release cannot affect other appearance owners. */
  #record(element: HTMLElement, name: string, value: string): void {
    let properties = this.#next.get(element);
    if (!properties) this.#next.set(element, (properties = new Map()));
    properties.set(name, value);
  }
}

/** Release stale owned values while preserving subsequent native or custom changes. */
function clearRemoved(previous: ElementAnnotations, next: ElementAnnotations): void {
  for (const [element, properties] of previous)
    for (const [name, value] of properties) {
      if (next.get(element)?.has(name)) continue;
      if (name.startsWith('--')) {
        if (element.style.getPropertyValue(name) === value) element.style.removeProperty(name);
      } else if (element.getAttribute(name) === value) element.removeAttribute(name);
    }
}
