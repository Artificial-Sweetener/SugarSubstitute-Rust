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
/** Render an accessible keyed library/cart with safe text, lifted-card dragging and keyboard ordering. */
import { computed, defineComponent, h, ref, TransitionGroup, watch, type PropType } from 'vue';
import { fixedOverlayOrigin } from '../../interaction/reorder/ReorderOverlayOrigin.js';
import { strideProjection } from '../../interaction/reorder/ReorderSlots.js';
import { useReorderGesture } from '../../interaction/reorder/vue/useReorderGesture.js';
import { CUBE_STACK_ROW_GAP, CUBE_STACK_ROW_HEIGHT } from '../../surface/stack/CubeStackMetrics.js';
import { CubeStackRow } from '../../surface/stack/vue/CubeStackRow.js';
import { useCubeStackFocus } from '../../surface/stack/vue/CubeStackFocus.js';
import { searchInputTag } from '../../surface/appearance/ControlFactory.js';
import type { CubeStackRowModel } from '../../surface/stack/CubeStackView.js';
import type {
  CubeStackCartViewActions,
  CubeStackCartViewState,
  CubeStackLibraryRow,
} from '../CubeStackCartView.js';

/** Marks a lifted library Cube in a previewed order: it is a copy on its way in, not a cart row. */
const INCOMING = 'incoming:';
const STRIDE = CUBE_STACK_ROW_HEIGHT + CUBE_STACK_ROW_GAP;
/** How far outside the cart's list the pointer may stray and still place or drop a Cube. */
const REACH = 24;

/**
 * Keep search and the transient drag preview inside presentation while the session owns accepted
 * draft edits. Cart rows and library Cubes are lifted with the gesture every reorderable list
 * shares: a cart row changes place, a library Cube is carried into the cart as a copy.
 */
export const CubeStackCart = defineComponent({
  name: 'SugarCubesStackCart',
  props: {
    state: { type: Object as PropType<CubeStackCartViewState>, required: true },
    actions: { type: Object as PropType<CubeStackCartViewActions>, required: true },
  },
  setup(props) {
    const root = ref<HTMLElement | null>(null);
    useCubeStackFocus(root);
    const query = ref('');
    const selected = ref<string | null>(null);
    const status = ref('');
    /** Where the dialog around the cart measures a fixed overlay from, read at each lift. */
    const overlayOrigin = ref({ x: 0, y: 0 });
    const cartList = (): HTMLElement | null =>
      root.value?.querySelector<HTMLElement>('.sc-cube-stack-cart__drop .sc-cube-stack__rows') ??
      null;
    const dropArea = (): HTMLElement | null =>
      root.value?.querySelector<HTMLElement>('.sc-cube-stack-cart__drop') ?? null;
    const overCart = (x: number, y: number): boolean => {
      const bounds = dropArea()?.getBoundingClientRect();
      return (
        !!bounds &&
        x >= bounds.left - REACH &&
        x <= bounds.right + REACH &&
        y >= bounds.top &&
        y <= bounds.bottom
      );
    };
    const gesture = useReorderGesture({
      root,
      threshold: 6,
      admission: () => (props.state.pending ? 'refuse' : 'ready'),
      keys: () => props.state.rows.map((row) => row.id),
      project: (card, current) => {
        const list = cartList();
        if (!list) return null;
        const listStart = list.getBoundingClientRect().top;
        const center = card.y + CUBE_STACK_ROW_HEIGHT / 2;
        if (!card.key.startsWith(INCOMING))
          return strideProjection({
            current,
            movable: () => true,
            key: card.key,
            center,
            listStart,
            stride: STRIDE,
            size: CUBE_STACK_ROW_HEIGHT,
          });
        // A library Cube takes the slot its centre is in while over the cart, and none elsewhere.
        const projected = current.filter((id) => id !== card.key);
        if (overCart(card.x + card.grabX, card.y + card.grabY)) {
          const slot = Math.floor((center - listStart) / STRIDE);
          projected.splice(Math.max(0, Math.min(projected.length, slot)), 0, card.key);
        }
        return projected.every((id, index) => id === current[index]) &&
          projected.length === current.length
          ? null
          : projected;
      },
      accepts: overCart,
      scroll: { viewport: dropArea, edge: 32, step: 8, lateral: REACH },
      lifted: (key) => {
        if (root.value) overlayOrigin.value = fixedOverlayOrigin(root.value);
        status.value = `Moving ${carried(key)?.label ?? 'Cube'}.`;
      },
      preview: () => undefined,
      commit: (key, order) => {
        if (key.startsWith(INCOMING)) {
          props.actions.add(key.slice(INCOMING.length), order[order.indexOf(key) + 1] ?? null);
          status.value = 'Cube added to the cart.';
        } else {
          props.actions.reorder(order);
          status.value = 'Cart reordered.';
        }
      },
      cancelled: (wasLifted) => {
        if (wasLifted) status.value = 'Move canceled.';
      },
    });
    /** The row a key names: a cart row, or the library Cube being carried in. */
    const carried = (key: string): CubeStackRowModel | null =>
      key.startsWith(INCOMING)
        ? (props.state.library.find((row) => row.id === key.slice(INCOMING.length)) ?? null)
        : (props.state.rows.find((row) => row.id === key) ?? null);
    const liftedRow = computed(() => {
      const card = gesture.card.value;
      return card ? carried(card.key) : null;
    });
    watch(
      () => props.state.pending,
      (pending) => {
        if (pending) gesture.clear();
      },
    );
    const rows = computed((): readonly CubeStackRowModel[] => {
      const order = gesture.order.value;
      if (!order) return props.state.rows;
      return order.flatMap((id) => {
        const row = carried(id);
        // The incoming copy is drawn under its preview key so it never collides with a cart row.
        return row ? [id.startsWith(INCOMING) ? { ...row, id } : row] : [];
      });
    });
    const groups = computed(() => {
      const found = props.state.library.filter((row) =>
        row.searchText.includes(query.value.trim().toLocaleLowerCase()),
      );
      const models = [...new Set(found.flatMap((row) => row.models))].sort((a, b) =>
        a.localeCompare(b),
      );
      const roles = ['Start cubes', 'Middle cubes', 'End cubes', 'Other cubes'] as const;
      return models.map((model) => ({
        model,
        roles: roles
          .map((role) => ({
            role,
            rows: found.filter((row) => row.models.includes(model) && row.role === role),
          }))
          .filter((group) => group.rows.length > 0),
      }));
    });
    const keyboard = (event: KeyboardEvent, row: CubeStackRowModel): void => {
      if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
      event.preventDefault();
      const delta = event.key === 'ArrowUp' ? -1 : 1;
      const index = props.state.rows.findIndex((item) => item.id === row.id);
      if (event.altKey) {
        if (props.state.pending || !props.state.rows[index + delta]) return;
        const order = props.state.rows.map((item) => item.id);
        order.splice(index, 1);
        order.splice(index + delta, 0, row.id);
        props.actions.reorder(order);
        status.value = `${row.label} moved ${delta < 0 ? 'up' : 'down'}.`;
      } else {
        const item = event.currentTarget;
        if (item instanceof HTMLElement)
          item.parentElement
            ?.querySelectorAll<HTMLButtonElement>('.sc-cube-stack-row__activate')
            [index + delta]?.focus();
      }
    };
    const renderRow = (row: CubeStackRowModel, library: boolean) =>
      h(CubeStackRow, {
        key: row.id,
        row: { ...row, selected: selected.value === row.id },
        pending: props.state.pending,
        removable: !library,
        draggable: false,
        'data-dragging': (!library && gesture.card.value?.key === row.id) || undefined,
        onPointerdown: (event: PointerEvent) => {
          // The remove button is pressed, never dragged.
          const pressed = event.target;
          if (!(pressed instanceof Element && pressed.closest('.sc-cube-stack-row__remove')))
            gesture.press(event, library ? `${INCOMING}${row.id}` : row.id);
        },
        onKeydown: library ? undefined : (event: KeyboardEvent) => keyboard(event, row),
        activate: () => {
          if (library) {
            props.actions.add(row.id);
            status.value = `${row.label} added to the cart.`;
          } else selected.value = row.id;
        },
        remove: () => {
          props.actions.remove(row.id);
          status.value = `${row.label} removed from the cart.`;
        },
        contextMenu: () => {
          selected.value = row.id;
        },
      });
    return () =>
      h(
        'div',
        {
          ref: root,
          class: 'sc-cube-stack-cart sc-cube-stack',
          'data-appearance': props.state.appearance,
          'aria-busy': props.state.pending || undefined,
          'data-pointer-dragging': gesture.card.value ? '' : undefined,
          onPointerdownCapture: gesture.beginInteraction,
          onPointercancel: gesture.cancel,
          onLostpointercapture: gesture.cancel,
          onClickCapture: gesture.click,
          onDragstartCapture: (event: DragEvent) => {
            event.preventDefault();
            event.stopPropagation();
          },
        },
        [
          h('div', { class: 'sc-cube-stack-cart__header' }, [
            h('p', { class: 'sc-cube-stack-cart__library-label' }, 'Cube library'),
            h(searchInputTag(props.state.appearance), {
              class: 'sc-cube-stack-cart__search',
              type: 'search',
              placeholder: 'Search cubes',
              'aria-label': 'Search cubes',
              label: 'Search cubes',
              value: query.value,
              onInput: (event: Event) => {
                const target = event.currentTarget;
                if (target instanceof HTMLElement) {
                  const value: unknown = Reflect.get(target, 'value');
                  if (typeof value === 'string') query.value = value;
                }
              },
            }),
          ]),
          h(
            'section',
            {
              class: 'sc-cube-stack-cart__pane sc-cube-stack',
              'data-kind': 'cart',
              'data-appearance': props.state.appearance,
              'aria-label': 'Cube library',
            },
            [
              h(
                'div',
                { class: 'sc-cube-stack-cart__scroll', 'data-capture-wheel': 'true' },
                groups.value.length
                  ? groups.value.map((group) =>
                      h('section', { key: group.model }, [
                        h('h3', { class: 'sc-cube-stack-cart__model' }, group.model),
                        ...group.roles.map((role) =>
                          h('section', { key: role.role }, [
                            h('h4', { class: 'sc-cube-stack-cart__role' }, role.role),
                            h(
                              'ul',
                              { class: 'sc-cube-stack__rows' },
                              role.rows.map((row: CubeStackLibraryRow) => renderRow(row, true)),
                            ),
                          ]),
                        ),
                      ]),
                    )
                  : [h('p', { class: 'sc-cube-stack-cart__empty' }, 'No cubes found.')],
              ),
            ],
          ),
          h(
            'section',
            {
              class: 'sc-cube-stack-cart__pane sc-cube-stack',
              'data-kind': 'cart',
              'data-appearance': props.state.appearance,
              'aria-label': 'Cube cart',
            },
            [
              h(
                'h3',
                { class: 'sc-cube-stack-cart__heading' },
                `Cube stack (${props.state.rows.length})`,
              ),
              h(
                'div',
                {
                  class: 'sc-cube-stack-cart__scroll sc-cube-stack-cart__drop',
                  'data-capture-wheel': 'true',
                },
                [
                  h(
                    TransitionGroup,
                    { tag: 'ul', class: 'sc-cube-stack__rows', name: 'sc-cube-stack' },
                    { default: () => rows.value.map((row) => renderRow(row, false)) },
                  ),
                  h(
                    'div',
                    { class: 'sc-cube-stack-cart__empty', 'aria-hidden': 'true' },
                    props.state.rows.length
                      ? 'Drag cubes here to add more'
                      : 'Click or drag cubes here',
                  ),
                ],
              ),
            ],
          ),
          gesture.card.value && liftedRow.value
            ? h(
                'div',
                {
                  class: 'sc-cube-stack sc-cube-stack__drag-overlay',
                  'data-kind': 'cart',
                  'data-appearance': props.state.appearance,
                  'aria-hidden': 'true',
                  inert: true,
                  style: {
                    width: `${String(gesture.card.value.width)}px`,
                    transform: `translate3d(${String(gesture.card.value.x - overlayOrigin.value.x)}px,${String(gesture.card.value.y - overlayOrigin.value.y)}px,0)`,
                  },
                },
                [
                  h(CubeStackRow, {
                    row: liftedRow.value,
                    style: {
                      transformOrigin: `${String(gesture.card.value.grabX)}px ${String(gesture.card.value.grabY)}px`,
                    },
                    activate: () => undefined,
                    remove: () => undefined,
                    contextMenu: () => undefined,
                  }),
                ],
              )
            : null,
          h(
            'span',
            { class: 'sc-cube-stack__status', role: 'status', 'aria-live': 'polite' },
            status.value,
          ),
        ],
      );
  },
});
