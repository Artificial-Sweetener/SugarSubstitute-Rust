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
/** Own Fluent advanced motion across projected native component replacements. */
import type { CardDisclosureMotion } from '../motion/CardDisclosureMotion.js';
import type { NativeProjectedAdvancedInputMount } from '../NativeProjectedAdvancedInputs.js';
import type { FluentCardBoundary } from './NativeFluentCardAdapter.js';
import { settleNativeDisclosure } from '../NativeDisclosureSettlement.js';
import { measureProjectedAdvancedHeight } from './NativeAdvancedDisclosureGeometry.js';

interface ProjectedBinding {
  motion: CardDisclosureMotion;
  unsubscribe(): void;
}

/** Retain one animation on the stable mount and delegate all native changes to the exact-node owner. */
export class NativeProjectedAdvancedDisclosure {
  readonly #bindings = new Map<HTMLElement, ProjectedBinding>();
  readonly #releasing = new Set<HTMLElement>();

  /** Preserve in-flight presentation when the native root and its widgets are replaced. */
  reconcile(mount: NativeProjectedAdvancedInputMount, card: FluentCardBoundary): void {
    if (this.#releasing.has(mount.target)) return;
    if (!card.advanced || !card.root.hasAttribute('data-sc-fluent-node')) {
      this.#release(mount.target);
      return;
    }
    if (this.#bindings.has(mount.target)) return;
    const motion = mount.motion;
    const unsubscribe = mount.present(
      () =>
        motion.toggle(
          !mount.control.shown(),
          (collapsed) => mount.control.setShown(!collapsed),
          {
            contentLayout: 'fill',
            closedHeight: () => measureProjectedAdvancedHeight(mount.target, true),
            expandedHeight: () => measureProjectedAdvancedHeight(mount.target, false),
            settle: () => settleNativeDisclosure(mount.target),
          },
          mount.control,
        ),
      () => this.#release(mount.target),
    );
    this.#bindings.set(mount.target, { motion, unsubscribe });
  }

  /** Commit the latest request and detach every projected presentation when Fluent ends. */
  clear(): void {
    for (const target of [...this.#bindings.keys()]) this.#release(target);
  }

  /** Release before committing state so a synchronous remount cannot reinstall a disposed binding. */
  #release(target: HTMLElement): void {
    const binding = this.#bindings.get(target);
    if (!binding) return;
    this.#bindings.delete(target);
    this.#releasing.add(target);
    try {
      binding.unsubscribe();
      binding.motion.dispose();
    } finally {
      this.#releasing.delete(target);
    }
  }
}
