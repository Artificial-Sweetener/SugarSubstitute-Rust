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
/** Adapt native advanced-field disclosure to the shared Fluent accordion motion. */
import { CardDisclosureMotion } from '../motion/CardDisclosureMotion.js';
import type { FluentCardBoundary } from './NativeFluentCardAdapter.js';
import { NATIVE_NODE_CARD_DISPOSE_EVENT } from '../NativeNodeCardRenderer.js';
import { settleNativeDisclosure } from '../NativeDisclosureSettlement.js';

interface AdvancedBinding {
  fields: HTMLElement;
  button: HTMLButtonElement;
  expanded: boolean;
  motion: CardDisclosureMotion;
  release(): void;
}

/** Preserve native advanced state and callbacks while owning only disclosure presentation. */
export class NativeAdvancedDisclosure {
  readonly #bindings = new Map<HTMLElement, AdvancedBinding>();

  /** Follow native component replacement and the current advanced state. */
  reconcile(card: FluentCardBoundary): void {
    for (const [root, binding] of this.#bindings) {
      if (!root.isConnected || (root === card.root && binding.fields !== card.advanced?.fields)) {
        binding.release();
        this.#bindings.delete(root);
      }
    }
    if (!card.advanced) return;
    const existing = this.#bindings.get(card.root);
    if (existing) {
      existing.button = card.advanced.button;
      existing.expanded = card.advanced.expanded;
      return;
    }
    const { fields, button, expanded } = card.advanced;
    let forwarding = false;
    let dispatching = false;
    const dispose = (): void => {
      binding.release();
      this.#bindings.delete(card.root);
    };
    const binding: AdvancedBinding = {
      fields,
      button,
      expanded,
      motion: new CardDisclosureMotion(fields),
      release: () => {
        card.root.removeEventListener('click', click, true);
        card.root.removeEventListener(NATIVE_NODE_CARD_DISPOSE_EVENT, dispose);
        binding.motion.dispose();
      },
    };
    const click = (event: Event): void => {
      if (
        forwarding ||
        !(event.target instanceof Element) ||
        !binding.button.contains(event.target)
      )
        return;
      event.stopImmediatePropagation();
      dispatching = true;
      try {
        binding.motion.toggle(
          !binding.expanded,
          (collapsed) => {
            if (binding.expanded === !collapsed) return;
            // Native HTMLElement.click suppresses reentrant clicks on the same button.
            const invoke = (): void => {
              if (binding.expanded === !collapsed) return;
              forwarding = true;
              try {
                binding.button.click();
              } finally {
                forwarding = false;
              }
              binding.expanded = !collapsed;
            };
            if (dispatching) queueMicrotask(invoke);
            else invoke();
          },
          {
            closedHeight: () => measureNativeFields(fields, true),
            expandedHeight: () => measureNativeFields(fields, false),
            settle: () => settleNativeDisclosure(fields),
          },
        );
      } finally {
        dispatching = false;
      }
    };
    card.root.addEventListener('click', click, true);
    card.root.addEventListener(NATIVE_NODE_CARD_DISPOSE_EVENT, dispose);
    this.#bindings.set(card.root, binding);
  }

  /** Commit pending native choices and release transient motion when appearance ends. */
  clear(): void {
    for (const binding of this.#bindings.values()) binding.release();
    this.#bindings.clear();
  }
}

/** Measure the native basic fields without changing widget values or advanced flags. */
function measureNativeFields(fields: HTMLElement, closed: boolean): number {
  const rows = closed
    ? [...fields.querySelectorAll<HTMLElement>(':scope > [data-sc-fluent-advanced-row]')]
    : [];
  const styles = rows.map((row) => ({
    value: row.style.getPropertyValue('display'),
    priority: row.style.getPropertyPriority('display'),
  }));
  const height = fields.style.getPropertyValue('height');
  const priority = fields.style.getPropertyPriority('height');
  try {
    fields.style.removeProperty('height');
    for (const row of rows) row.style.display = 'none';
    const intrinsic = Number.parseFloat(
      fields.ownerDocument.defaultView?.getComputedStyle(fields).height ?? '',
    );
    return Number.isFinite(intrinsic) ? intrinsic : fields.offsetHeight;
  } finally {
    fields.style.setProperty('height', height, priority);
    rows.forEach((row, index) => {
      const style = styles[index];
      row.style.setProperty('display', style?.value ?? '', style?.priority ?? '');
    });
  }
}
