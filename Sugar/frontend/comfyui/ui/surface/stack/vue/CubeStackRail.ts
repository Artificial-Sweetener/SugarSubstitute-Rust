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
/** Present a stable Vue navigation rail with canonical drag constraints and keyboard equivalents. */
import { defineComponent, h, ref, computed, watch, type PropType } from 'vue';
import { buttonTag } from '../../appearance/ControlFactory.js';
import { CUBE_STACK_ROW_HEIGHT, cubeStackRowGap, cubeStackRowStride } from '../CubeStackMetrics.js';
import type {
  CubeStackNavigationRow,
  CubeStackViewActions,
  CubeStackViewState,
} from '../CubeStackView.js';
import { CubeStackRow } from './CubeStackRow.js';
import { useCubeStackFocus } from './CubeStackFocus.js';
import { strideProjection } from '../../../interaction/reorder/ReorderSlots.js';
import { useReorderGesture } from '../../../interaction/reorder/vue/useReorderGesture.js';
import { CubeStackSelectionIndicator } from './CubeStackSelectionIndicator.js';

/** Animate temporary row order while keeping actual graph mutation at the application boundary. */
export const CubeStackRail = defineComponent({
  name: 'SugarCubesStackRail',
  props: {
    interactive: { type: Boolean, default: true },
    state: { type: Object as PropType<CubeStackViewState>, required: true },
    actions: { type: Object as PropType<CubeStackViewActions>, required: true },
  },
  setup(props) {
    const root = ref<HTMLElement | null>(null);
    const announcement = ref('');
    const awaiting = ref(false);
    const submittedOrder = ref<readonly string[] | null>(null);
    useCubeStackFocus(root);
    const rowStride = computed(() => cubeStackRowStride(props.state.packed));
    const busy = (): boolean => props.state.pending || awaiting.value;
    /** How far beside the rail the pointer may stray and still move or drop a Cube. */
    const REACH = 24;
    const beside = (x: number): boolean => {
      const bounds = root.value?.getBoundingClientRect();
      return !!bounds && x >= bounds.left - REACH && x <= bounds.right + REACH;
    };
    const gesture = useReorderGesture({
      root,
      threshold: 6,
      // A press made while an earlier drop is still being applied waits for it.
      admission: (id) =>
        busy()
          ? 'defer'
          : props.state.rows.find((row) => row.id === id)?.reorderable
            ? 'ready'
            : 'refuse',
      keys: () => props.state.rows.map((row) => row.id),
      project: (card, current) => {
        const list = root.value?.querySelector<HTMLElement>('.sc-cube-stack__rows');
        if (!list) return null;
        const movable = new Set(
          props.state.rows.filter((row) => row.reorderable).map((row) => row.id),
        );
        return strideProjection({
          current,
          movable: (id) => movable.has(id),
          key: card.key,
          center: card.y + CUBE_STACK_ROW_HEIGHT / 2,
          listStart: list.getBoundingClientRect().top,
          stride: rowStride.value,
          size: CUBE_STACK_ROW_HEIGHT,
        });
      },
      projects: (x) => beside(x),
      accepts: (x, y) => {
        const viewport = root.value?.parentElement?.getBoundingClientRect();
        return !!viewport && beside(x) && y >= viewport.top && y <= viewport.bottom;
      },
      scroll: {
        viewport: () => root.value?.parentElement ?? null,
        edge: 32,
        step: 8,
        lateral: REACH,
      },
      lifted: (id) => {
        const label = props.state.rows.find((row) => row.id === id)?.label ?? '';
        announcement.value = `Moving ${label}. Drop at any Cube position.`;
      },
      preview: props.actions.previewOrder,
      commit: (id, order) => {
        submittedOrder.value = order;
        void execute(() => props.actions.reorder(id, order), 'Cube moved.');
      },
      cancelled: (wasLifted) => {
        if (wasLifted) announcement.value = 'Move canceled.';
      },
    });
    /** The row in the air, as the accepted state describes it. */
    const liftedRow = computed(
      () => props.state.rows.find((row) => row.id === gesture.card.value?.key) ?? null,
    );
    watch(
      () => props.interactive,
      (interactive) => {
        if (!interactive) gesture.clear();
      },
      { flush: 'sync' },
    );
    const rows = computed(() => {
      const order = gesture.order.value ?? submittedOrder.value;
      if (!order) return props.state.rows;
      return [...props.state.rows].sort(
        (left, right) => order.indexOf(left.id) - order.indexOf(right.id),
      );
    });
    const clearPreview = (): void => {
      submittedOrder.value = null;
      props.actions.previewOrder(null);
    };
    watch(
      () => props.state.rows.map((row) => row.id).join('|'),
      () => {
        if (!awaiting.value && gesture.card.value) gesture.clear();
      },
    );
    watch(
      () => props.state.pending || awaiting.value,
      (pending) => {
        if (!pending) gesture.resume();
      },
    );
    const execute = async (work: () => Promise<void>, label: string): Promise<void> => {
      awaiting.value = true;
      try {
        await work();
        announcement.value = label;
      } catch (error: unknown) {
        props.actions.reportError(error);
      } finally {
        awaiting.value = false;
        clearPreview();
      }
    };
    const keyboard = (event: KeyboardEvent, row: CubeStackNavigationRow): void => {
      if (!props.interactive) return;
      if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
      const delta = event.key === 'ArrowUp' ? -1 : 1;
      event.preventDefault();
      if (event.altKey) {
        const group = props.state.rows;
        if (
          !props.state.pending &&
          !awaiting.value &&
          row.reorderable &&
          group[group.indexOf(row) + delta]
        )
          void execute(
            () => props.actions.move(row.id, delta),
            `${row.label} moved ${delta < 0 ? 'up' : 'down'}.`,
          );
        return;
      }
      const buttons = root.value?.querySelectorAll<HTMLButtonElement>(
        '.sc-cube-stack-row__activate',
      );
      const index = rows.value.indexOf(row) + delta;
      buttons?.[index]?.focus({ preventScroll: false });
    };
    const tool = (icon: string, label: string, action: () => void) =>
      h(
        buttonTag(props.state.appearance),
        {
          class: 'sc-cube-stack__tool',
          type: 'button',
          appearance: 'subtle',
          'data-sc-button': '',
          quiet: '',
          size: props.state.appearance === 'spectrum' ? 's' : 'small',
          'icon-only': '',
          title: label,
          'aria-label': label,
          onClick: () => {
            if (props.interactive) action();
          },
        },
        [h('i', { class: `pi ${icon}`, 'aria-hidden': 'true' })],
      );
    return () =>
      h(
        'nav',
        {
          ref: root,
          class: 'sc-cube-stack',
          'aria-label': 'Cube stack',
          'aria-busy': props.state.pending || undefined,
          'data-appearance': props.state.appearance,
          'data-kind': 'rail',
          'data-compact': props.state.compact || undefined,
          'data-capture-wheel': 'true',
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
          h('div', { class: 'sc-cube-stack__toolbar' }, [
            tool(
              props.state.compact ? 'pi-angle-right' : 'pi-angle-left',
              props.state.compact ? 'Expand Cube stack' : 'Compact Cube stack',
              () => props.actions.setCompact(!props.state.compact),
            ),
            tool('pi-times', 'Hide Cube stack', props.actions.hide),
          ]),
          h(
            'ul',
            {
              class: 'sc-cube-stack__rows',
              style: {
                height: `${String(Math.max(0, rows.value.length * rowStride.value - cubeStackRowGap(props.state.packed)))}px`,
              },
            },
            [
              h(CubeStackSelectionIndicator, {
                key: 'selection',
                packed: props.state.packed,
                position: rows.value.some((row) => row.selected)
                  ? rows.value.findIndex((row) => row.selected)
                  : null,
              }),
              ...rows.value.map((row, index) =>
                h(CubeStackRow, {
                  key: row.id,
                  row,
                  style: { transform: `translateY(${String(index * rowStride.value)}px)` },
                  compact: props.state.compact,
                  pending: props.state.pending || !props.interactive,
                  removable: !props.state.compact,
                  draggable: false,
                  'data-reorderable':
                    (row.reorderable && !props.state.pending && props.interactive) || undefined,
                  'data-dragging': gesture.card.value?.key === row.id || undefined,
                  onPointerdown: (event: PointerEvent) => {
                    // The remove button is pressed, never dragged.
                    const pressed = event.target;
                    if (
                      props.interactive &&
                      !(pressed instanceof Element && pressed.closest('.sc-cube-stack-row__remove'))
                    )
                      gesture.press(event, row.id);
                  },
                  onKeydown: (event: KeyboardEvent) => keyboard(event, row),
                  activate: () => {
                    if (!gesture.card.value) props.actions.navigate(row.id);
                  },
                  remove: () => props.actions.remove(row.id),
                  contextMenu: (event: MouseEvent) => props.actions.contextMenu(row.id, event),
                }),
              ),
            ],
          ),
          gesture.card.value && liftedRow.value
            ? h(
                'div',
                {
                  class: 'sc-cube-stack__drag-overlay',
                  'aria-hidden': 'true',
                  inert: true,
                  style: {
                    width: `${String(gesture.card.value.width)}px`,
                    transform: `translate3d(${String(gesture.card.value.x)}px,${String(gesture.card.value.y)}px,0)`,
                  },
                },
                [
                  h(CubeStackRow, {
                    row: liftedRow.value,
                    compact: props.state.compact,
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
            buttonTag(props.state.appearance),
            {
              class: 'sc-cube-stack__add',
              type: 'button',
              appearance: 'subtle',
              'data-sc-button': '',
              quiet: '',
              'aria-label': 'Add cubes',
              title: 'Add cubes',
              disabled: props.state.pending || !props.interactive,
              onClick: () => {
                if (props.interactive) props.actions.openCart();
              },
            },
            [h('i', { class: 'pi pi-plus', 'aria-hidden': 'true' })],
          ),
          h(
            'span',
            { class: 'sc-cube-stack__status', role: 'status', 'aria-live': 'polite' },
            announcement.value,
          ),
        ],
      );
  },
});
