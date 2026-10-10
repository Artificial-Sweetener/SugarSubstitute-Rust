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
/** Render an accessible keyed library/cart with safe text, draft drag preview and keyboard ordering. */
import { computed, defineComponent, h, ref, TransitionGroup, type PropType } from 'vue';
import { CubeStackRow } from '../../surface/stack/vue/CubeStackRow.js';
import { useCubeStackFocus } from '../../surface/stack/vue/CubeStackFocus.js';
import type { CubeStackRowModel } from '../../surface/stack/CubeStackView.js';
import type {
  CubeStackCartViewActions,
  CubeStackCartViewState,
  CubeStackLibraryRow,
} from '../CubeStackCartView.js';

/** Keep search and transient pointer preview inside presentation while the session owns accepted draft edits. */
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
    const dragged = ref<string | null>(null);
    const artifact = ref<string | null>(null);
    const preview = ref<readonly string[] | null>(null);
    const selected = ref<string | null>(null);
    const status = ref('');
    const rows = computed(() => {
      const order = preview.value;
      return order
        ? [...props.state.rows].sort(
            (left, right) => order.indexOf(left.id) - order.indexOf(right.id),
          )
        : props.state.rows;
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
    const clear = (): void => {
      dragged.value = null;
      artifact.value = null;
      preview.value = null;
    };
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
    const start = (event: DragEvent, row: CubeStackRowModel, fromLibrary: boolean): void => {
      if (props.state.pending || !event.dataTransfer) {
        event.preventDefault();
        return;
      }
      clear();
      if (fromLibrary) artifact.value = row.id;
      else {
        dragged.value = row.id;
        preview.value = props.state.rows.map((item) => item.id);
      }
      event.dataTransfer.effectAllowed = fromLibrary ? 'copy' : 'move';
      event.dataTransfer.setData('application/x-sugarcubes-cart', row.id);
    };
    const over = (event: DragEvent, row: CubeStackRowModel | null): void => {
      if (props.state.pending || (!dragged.value && !artifact.value)) return;
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = artifact.value ? 'copy' : 'move';
      if (!dragged.value || !preview.value || row?.id === dragged.value) return;
      const order = preview.value.filter((id) => id !== dragged.value);
      const element = event.currentTarget;
      const bounds = element instanceof HTMLElement ? element.getBoundingClientRect() : null;
      const index = row ? order.indexOf(row.id) : order.length;
      const after = row && bounds && event.clientY >= bounds.top + bounds.height / 2;
      order.splice(index + (after ? 1 : 0), 0, dragged.value);
      preview.value = order;
    };
    const drop = (event: DragEvent, row: CubeStackRowModel | null): void => {
      if (props.state.pending || (!dragged.value && !artifact.value)) return;
      event.preventDefault();
      event.stopPropagation();
      if (artifact.value) props.actions.add(artifact.value, row?.id ?? null);
      else if (preview.value) props.actions.reorder(preview.value);
      status.value = artifact.value ? 'Cube added to the cart.' : 'Cart reordered.';
      clear();
    };
    const renderRow = (row: CubeStackRowModel, library: boolean) =>
      h(CubeStackRow, {
        key: row.id,
        row: { ...row, selected: selected.value === row.id },
        pending: props.state.pending,
        removable: !library,
        draggable: !props.state.pending,
        'data-dragging': dragged.value === row.id || undefined,
        onDragstart: (event: DragEvent) => start(event, row, library),
        onDragend: clear,
        onDragover: library
          ? undefined
          : (event: DragEvent) => {
              event.stopPropagation();
              over(event, row);
            },
        onDrop: library ? undefined : (event: DragEvent) => drop(event, row),
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
        },
        [
          h('div', { class: 'sc-cube-stack-cart__header' }, [
            h('p', { class: 'sc-cube-stack-cart__library-label' }, 'Cube library'),
            h(props.state.appearance === 'fluent' ? 'fluent-text-input' : 'input', {
              class: 'sc-cube-stack-cart__search',
              type: 'search',
              placeholder: 'Search cubes',
              'aria-label': 'Search cubes',
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
                  onDragover: (event: DragEvent) => over(event, null),
                  onDrop: (event: DragEvent) => drop(event, null),
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
          h(
            'span',
            { class: 'sc-cube-stack__status', role: 'status', 'aria-live': 'polite' },
            status.value,
          ),
        ],
      );
  },
});
