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
/** Literal MutationObserver transport records actual external DOM calls made by supported Widget callbacks. */
export function visibilityObserverFixture(document: Document) {
  const observers: Observer[] = [],
    instrumented = new WeakSet<HTMLElement>();
  let onObserve: ((target: Node) => void) | null = null;
  /** Native queue ownership belongs to one observed target until disconnect. */
  class Observer {
    target: Node | null = null;
    options: MutationObserverInit = {};
    readonly records: MutationRecord[] = [];
    /** Retain the actual callback installed by Resource. */
    readonly callback: MutationCallback;
    constructor(callback: MutationCallback) {
      this.callback = callback;
      observers.push(this);
    }
    /** Observe actual ancestry without inventing records from masks. */
    observe(target: Node, options: MutationObserverInit): void {
      this.target = target;
      this.options = options;
      if (target.nodeType === 1) instrument(target as HTMLElement);
      onObserve?.(target);
    }
    /** Delivery removes only this observer's actual pending native records. */
    takeRecords(): MutationRecord[] {
      return this.records.splice(0);
    }
    /** Terminal disconnect releases both target and queued records. */
    disconnect(): void {
      this.target = null;
      this.records.splice(0);
    }
  }
  /** Node/name/oldValue originate from actual setter calls, including duplicate and foreign writes. */
  const emit = (
    node: Node,
    type: 'attributes' | 'childList' | 'characterData',
    attributeName: string | null,
    oldValue: string | null,
  ): void => {
    for (const observer of observers) {
      const target = observer.target;
      if (!target || !(target === node || (observer.options.subtree && target.contains(node))))
        continue;
      if (type === 'attributes' && !observer.options.attributes) continue;
      if (type === 'childList' && !observer.options.childList) continue;
      if (type === 'characterData' && !observer.options.characterData) continue;
      observer.records.push({
        type,
        target: node,
        attributeName,
        oldValue: observer.options.attributeOldValue ? oldValue : null,
      } as unknown as MutationRecord);
    }
  };
  /** Wrap only literal native transports, preserving the original returned/retired DOM. */
  function instrument(node: HTMLElement): HTMLElement {
    if (instrumented.has(node)) return node;
    instrumented.add(node);
    const set = node.setAttribute.bind(node),
      remove = node.removeAttribute.bind(node),
      append = node.append.bind(node);
    for (const property of ['height', 'flexBasis'] as const) {
      const descriptor = Object.getOwnPropertyDescriptor(node.style, property);
      if (descriptor?.get && descriptor.set)
        Object.defineProperty(node.style, property, {
          ...descriptor,
          set(value: string) {
            const old = node.getAttribute('style');
            descriptor.set!.call(node.style, value);
            if (node.getAttribute('style') !== old) emit(node, 'attributes', 'style', old);
          },
        });
    }
    node.setAttribute = (name, value) => {
      const old = node.getAttribute(name);
      set(name, value);
      emit(node, 'attributes', name, old);
    };
    node.removeAttribute = (name) => {
      const old = node.getAttribute(name);
      remove(name);
      if (old !== null) emit(node, 'attributes', name, old);
    };
    node.append = (...children) => {
      append(...children);
      if (children.length) emit(node, 'childList', null, null);
    };
    return node;
  }
  const createText = document.createTextNode.bind(document);
  Object.defineProperty(document, 'createTextNode', {
    configurable: true,
    /** Native characterData ownership observes actual writes, including same-length text replacement. */
    value: (value: string) => {
      const node = createText(value);
      let text = node.textContent ?? '';
      for (const property of ['data', 'textContent'] as const)
        Object.defineProperty(node, property, {
          configurable: true,
          get: () => text,
          set: (next: string) => {
            const old = text;
            text = next;
            emit(node, 'characterData', null, old);
          },
        });
      return node;
    },
  });
  const create = document.createElement.bind(document);
  Object.defineProperty(document, 'createElement', {
    configurable: true,
    value: (name: string) => instrument(create(name)),
  });
  Object.defineProperty(document.defaultView!, 'MutationObserver', {
    configurable: true,
    value: Observer,
  });
  return {
    /** Independently retain literal native writes for direct receipt characterization. */
    track(target: Node) {
      const observer = new Observer(() => {});
      observer.observe(target, {
        attributes: true,
        attributeOldValue: true,
        childList: true,
        subtree: true,
      });
      return {
        take: () => observer.takeRecords(),
        destroy: () => observer.disconnect(),
      };
    },
    /** Set a finite acquisition hook for pending-record/error ownership controls. */
    onObserve(callback: (target: Node) => void) {
      onObserve = callback;
    },
    /** Deliver complete actual callback batches without changing the producer's metadata. */
    deliver() {
      for (const observer of observers) {
        const records = observer.takeRecords();
        if (records.length && observer.target)
          observer.callback(records, observer as unknown as MutationObserver);
      }
    },
    /** A terminal Resource must disconnect every acquired native subscription. */
    get active() {
      return observers.filter((observer) => observer.target !== null).length;
    },
  };
}
