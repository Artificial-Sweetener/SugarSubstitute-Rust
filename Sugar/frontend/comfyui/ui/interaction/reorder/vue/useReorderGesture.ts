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
/** Give a Vue component the shared reorder gesture as reactive state. */
import { onBeforeUnmount, shallowRef, type Ref, type ShallowRef } from 'vue';
import { ReorderGesture, type LiftedCard, type ReorderGestureOptions } from '../ReorderGesture.js';

/** What a component decides about its list; the adapter supplies the page and the reactive state. */
export type ReorderGestureSetup = Omit<
  ReorderGestureOptions,
  'document' | 'captureElement' | 'moved'
> & {
  /** The component's root, which holds the pointer once a card is lifted. */
  readonly root: Ref<HTMLElement | null>;
};

/** The gesture as a component uses it. */
export interface ReorderGestureBinding {
  /** The card in the air, for drawing it and marking the place it left. */
  readonly card: ShallowRef<LiftedCard | null>;
  /** The order to draw while a card is in the air. */
  readonly order: ShallowRef<readonly string[] | null>;
  press(event: PointerEvent, key: string): void;
  resume(): void;
  cancel(): void;
  clear(): void;
  click(event: MouseEvent): void;
  beginInteraction(): void;
}

/**
 * Bind the shared gesture to one component for its lifetime. The gesture is made at the first
 * press, when the component's root is on a page, and released when the component unmounts.
 */
export function useReorderGesture(setup: ReorderGestureSetup): ReorderGestureBinding {
  const card = shallowRef<LiftedCard | null>(null);
  const order = shallowRef<readonly string[] | null>(null);
  let gesture: ReorderGesture | null = null;
  const made = (): ReorderGesture | null => {
    const root = setup.root.value;
    if (gesture || !root) return gesture;
    gesture = new ReorderGesture({
      ...setup,
      document: root.ownerDocument,
      captureElement: () => setup.root.value,
      moved: (next) => {
        card.value = next;
        // The order shown follows the card: the accepted order at the lift, none once it lands.
        order.value = next ? (gesture?.order ?? order.value) : null;
      },
      preview: (next) => {
        order.value = next ?? (card.value ? (gesture?.order ?? null) : null);
        setup.preview(next);
      },
    });
    return gesture;
  };
  onBeforeUnmount(() => gesture?.clear());
  return {
    card,
    order,
    press: (event, key) => {
      const item = event.currentTarget;
      if (item instanceof HTMLElement) made()?.press(event, key, item);
    },
    resume: () => gesture?.resume(),
    cancel: () => gesture?.cancel(),
    clear: () => gesture?.clear(),
    click: (event) => gesture?.click(event),
    beginInteraction: () => gesture?.beginInteraction(),
  };
}
