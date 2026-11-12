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
/** Own the editable banner field and its attached, host-independent model popup. */
import {
  computed,
  defineComponent,
  h,
  nextTick,
  onBeforeUnmount,
  ref,
  shallowRef,
  Teleport,
  useId,
  watch,
} from 'vue';
import type { PropType } from 'vue';
import { chooseModelThumbnail } from '../ModelCatalog.js';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelPickerBinding } from '../ModelPickerBinding.js';
import { filterModelChoices } from '../ModelPickerChoices.js';
import { queryModelCatalog, type ModelCompatibilityContext } from '../ModelCompatibility.js';
import { modelChoiceCompletion } from '../ModelPickerCompletion.js';
import { modelPickerPlacement } from '../ModelPickerPlacement.js';
import { ModelPickerPopupContent } from './ModelPickerPopupContent.js';
import type { ModelPickerActions, ModelPickerFeedback } from '../ModelPickerActions.js';
import { modelWallNeighbor } from '../ModelPickerNavigation.js';
import { useModelContext } from './ModelContextState.js';
import type { ModelUpdatePort } from '../ModelUpdates.js';
import type { ModelDetectorPort } from '../ModelDetectors.js';
import { ModelUpdateButton } from './ModelUpdateButton.js';
import { ModelCatalogImage } from './ModelCatalogImage.js';
import type { ModelOutputThumbnails } from '../ModelOutputThumbnails.js';
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import { ModelTransferView } from './ModelTransferView.js';
import { ModelPickerSession } from '../ModelPickerSession.js';
import type { ModelTransferState } from '../ModelTransfer.js';
import { useModelSelectionUpdates } from './ModelSelectionUpdates.js';
import { acquireComboToggle } from '../../surface/appearance/ComboToggleController.js';

/** Share the same searchable field in graph and masonry without owning workflow values. */
export const ModelPickerField = defineComponent({
  name: 'SugarCubesModelPickerField',
  props: {
    session: { type: Object as PropType<ModelPickerSession | null>, default: null },
    binding: { type: Object as PropType<ModelPickerBinding>, required: true },
    items: { type: Array as PropType<readonly ModelCatalogItem[]>, required: true },
    actions: { type: Object as PropType<ModelPickerActions | null>, default: null },
    feedback: { type: Function as PropType<ModelPickerFeedback | null>, default: null },
    updates: { type: Object as PropType<ModelUpdatePort | null>, default: null },
    detectors: { type: Object as PropType<ModelDetectorPort | null>, default: null },
    outputs: { type: Object as PropType<ModelOutputThumbnails | null>, default: null },
    discoveryAction: { type: Function as PropType<(() => void) | null>, default: null },
    compatibility: {
      type: Function as PropType<(() => ModelCompatibilityContext | null) | null>,
      default: null,
    },
  },
  emits: { contextReady: (_read: () => readonly ContextMenuEntry[]) => true },
  setup(props, { emit }) {
    const releaseComboToggle = acquireComboToggle(document);
    const session = props.session ?? new ModelPickerSession(document, () => props.binding);
    const transfer = shallowRef<ModelTransferState | null>(session.read());
    const unsubscribeTransfer = session.subscribe(() => {
      transfer.value = session.read();
    });
    const open = ref(false);
    const context = useModelContext(
      props,
      () => props.binding.disabled(),
      () => close(),
      session,
    );
    const query = ref('');
    const folder = ref('');
    const current = ref(props.binding.readValue());
    const active = ref(0);
    const input = ref<HTMLInputElement | null>(null);
    const surface = ref<HTMLElement | null>(null);
    const popup = ref<HTMLElement | null>(null);
    const placement = ref<Record<string, string>>({});
    const opensBelow = ref(true);
    const popupId = useId();
    const currentItem = computed(() => props.items.find((item) => item.value === current.value));
    useModelSelectionUpdates(() => currentItem.value, props.updates, props.feedback);
    emit('contextReady', () => (currentItem.value ? context.project(currentItem.value) : []));
    const compatibility = ref<ModelCompatibilityContext | null>(props.compatibility?.() ?? null);
    const unfiltered = ref(false);
    const compatible = computed(() =>
      queryModelCatalog(props.items, {
        compatibleWith: unfiltered.value ? null : compatibility.value,
      }),
    );
    const offersDiscovery = computed(() => !!props.discoveryAction && !compatible.value.length);
    const visible = computed(() => filterModelChoices(compatible.value, query.value, folder.value));
    const navigable = computed(() => filterModelChoices(compatible.value, query.value, ''));
    const wallWidth = ref(540);
    const completion = computed(() =>
      modelChoiceCompletion(visible.value[active.value], query.value),
    );
    const unsubscribe = props.binding.subscribe(() => {
      current.value = props.binding.readValue();
      compatibility.value = props.compatibility?.() ?? null;
    });
    let frame: number | null = null;

    /** Follow graph transforms using viewport coordinates only while the popup is open. */
    const position = (): void => {
      if (!open.value || !surface.value) return;
      const rectangle = surface.value.getBoundingClientRect();
      const geometry = modelPickerPlacement(
        { left: 0, top: 0, width: innerWidth, height: innerHeight },
        rectangle,
      );
      opensBelow.value = geometry.mode === 'below';
      const nextPlacement = {
        left: `${String(geometry.left)}px`,
        top: `${String(geometry.top)}px`,
        width: `${String(geometry.width)}px`,
        height: `${String(geometry.height)}px`,
      };
      if (Object.entries(nextPlacement).some(([key, value]) => placement.value[key] !== value))
        placement.value = nextPlacement;
      frame = requestAnimationFrame(position);
    };

    /** Dismiss temporary search without writing to the host widget. */
    const close = (): void => {
      context.menu.value = null;
      open.value = false;
      query.value = '';
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
    };

    watch(offersDiscovery, (offered) => {
      if (offered) close();
    });

    /** Commit exactly one permitted Comfy filename through its existing callback path. */
    const select = (value: string): void => {
      if (!props.binding.options.includes(value) || props.binding.disabled()) return;
      props.binding.writeValue(value);
      current.value = props.binding.readValue();
      close();
      input.value?.focus();
    };

    /** Start every browsing session at the root without changing the selected model. */
    const show = (): void => {
      if (open.value || transfer.value || props.binding.disabled()) return;
      compatibility.value = props.compatibility?.() ?? null;
      unfiltered.value = false;
      open.value = true;
      query.value = '';
      folder.value = '';
      active.value = Math.max(
        0,
        visible.value.findIndex((item) => item.value === current.value),
      );
      position();
      void nextTick(() => input.value?.focus());
    };

    /** Keep clicking elsewhere reversible and local to this picker lifetime. */
    const outside = (event: Event): void => {
      if (context.menu.value) return;
      if (
        event.target instanceof Node &&
        !surface.value?.contains(event.target) &&
        !popup.value?.contains(event.target)
      )
        close();
    };
    /** Dismiss inspection or search from any popup control without passing Escape to the graph. */
    const escape = (event: KeyboardEvent): void => {
      if (context.menu.value) return;
      if (!open.value || event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      close();
      void nextTick(() => input.value?.focus());
    };
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('keydown', escape, true);
    onBeforeUnmount(() => {
      releaseComboToggle();
      close();
      unsubscribe();
      unsubscribeTransfer();
      if (!props.session) session.dispose();
      document.removeEventListener('pointerdown', outside, true);
      document.removeEventListener('keydown', escape, true);
    });

    /** Route keyboard navigation without allowing the graph to handle editor gestures. */
    const key = (event: KeyboardEvent): void => {
      event.stopPropagation();
      if (event.key === 'Escape') {
        event.preventDefault();
        close();
        return;
      }
      if (!open.value && (event.key === 'Enter' || event.key === 'ArrowDown')) {
        event.preventDefault();
        show();
        return;
      }
      if (!open.value) return;
      if (event.key === 'Tab' && completion.value) {
        event.preventDefault();
        const highlighted = visible.value[active.value]?.value;
        query.value = completion.value.text;
        active.value = Math.max(
          0,
          visible.value.findIndex((item) => item.value === highlighted),
        );
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const item = visible.value[active.value];
        if (item) select(item.value);
      } else if (
        ['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)
      ) {
        event.preventDefault();
        active.value = modelWallNeighbor(visible.value, wallWidth.value, active.value, event.key);
      }
    };

    return () => {
      if (offersDiscovery.value && !transfer.value)
        return h(
          'button',
          {
            class: 'sc-model-find',
            type: 'button',
            disabled: props.binding.disabled(),
            onPointerdown: (event: Event) => event.stopPropagation(),
            onMousedown: (event: Event) => event.stopPropagation(),
            onClick: props.discoveryAction,
          },
          [h('span', { class: 'sc-model-find-icon', 'aria-hidden': 'true' }), 'Find models'],
        );
      const item = currentItem.value;
      const banner = item ? chooseModelThumbnail(item, 'banner', 768) : null;
      const label = item ? [item.title, item.version].filter(Boolean).join(' - ') : current.value;
      return h(
        'div',
        {
          ref: surface,
          class: [
            'sc-model-field',
            { 'sc-model-field-open': open.value, 'sc-model-field-transferring': !!transfer.value },
          ],
          'data-sc-style-combo-frame': '',
          'aria-busy': !!transfer.value,
          onPointerdown: (event: Event) => event.stopPropagation(),
          onMousedown: (event: Event) => event.stopPropagation(),
          onWheel: (event: Event) => event.stopPropagation(),
          onContextmenu: (event: MouseEvent) => {
            if (item) context.show(item, event);
          },
        },
        [
          transfer.value ? h(ModelTransferView, { transfer: transfer.value }) : null,
          banner && !open.value && !transfer.value
            ? h(ModelCatalogImage, { class: 'sc-model-banner', image: banner })
            : null,
          h('input', {
            ref: input,
            value: transfer.value
              ? `Downloading ${transfer.value.model.name}`
              : open.value
                ? query.value
                : label,
            readonly: !open.value,
            disabled: props.binding.disabled() || !!transfer.value,
            role: 'combobox',
            'aria-label': props.binding.label,
            'aria-expanded': open.value,
            'aria-controls': open.value ? popupId : undefined,
            'aria-activedescendant':
              open.value && visible.value[active.value]
                ? `${popupId}-option-${String(active.value)}`
                : undefined,
            'aria-autocomplete': 'both',
            title: transfer.value ? transfer.value.model.name : current.value,
            spellcheck: false,
            onClick: show,
            onKeydown: key,
            onInput: (event: Event) => {
              if (event.target instanceof HTMLInputElement) {
                query.value = event.target.value;
                active.value = 0;
              }
            },
          }),
          open.value && completion.value
            ? h('span', { class: 'sc-model-completion', 'aria-hidden': true }, [
                h('span', { class: 'sc-model-completion-prefix' }, query.value),
                completion.value.suffix,
              ])
            : null,
          item && !open.value && !transfer.value && context.offer(item)
            ? h(ModelUpdateButton, {
                onOpen: () => {
                  close();
                  context.family(item);
                },
                onContext: (event: MouseEvent) => context.show(item, event, true),
              })
            : null,
          h(
            'button',
            {
              type: 'button',
              class: 'sc-model-field-arrow',
              tabindex: -1,
              disabled: !!transfer.value,
              'aria-label': `Choose ${props.binding.label}`,
              onClick: show,
            },
            h('span', { class: 'sc-model-chevron' }),
          ),
          open.value
            ? h(
                Teleport,
                { to: 'body' },
                h(
                  'div',
                  {
                    ref: popup,
                    id: popupId,
                    class: ['sc-model-popup', { 'sc-model-popup-above': !opensBelow.value }],
                    style: placement.value,
                    onPointerdown: (event: Event) => event.stopPropagation(),
                    onWheel: (event: Event) => event.stopPropagation(),
                  },
                  [
                    h(ModelPickerPopupContent, {
                      items: navigable.value,
                      compatibilityLabel:
                        compatibility.value?.architecture || compatibility.value?.label || '',
                      unfiltered: unfiltered.value,
                      onToggleFilter: () => {
                        unfiltered.value = !unfiltered.value;
                        folder.value = '';
                        active.value = 0;
                        input.value?.focus();
                      },
                      visible: visible.value,
                      folder: folder.value,
                      selected: current.value,
                      active: active.value,
                      optionPrefix: popupId,
                      onNavigate: (path: string) => {
                        folder.value = path;
                        active.value = 0;
                        input.value?.focus();
                      },
                      onSelect: select,
                      onHighlight: (index: number) => {
                        active.value = index;
                      },
                      onGeometry: (width: number) => {
                        wallWidth.value = width;
                      },
                      updateOffers: context.proposals.value,
                      onFamily: (target: ModelCatalogItem) => {
                        close();
                        context.family(target);
                      },
                      onUpdateContext: (target: ModelCatalogItem, event: MouseEvent) =>
                        context.show(target, event, true),
                      onContext: context.show,
                    }),
                  ],
                ),
              )
            : null,
          context.render(),
        ],
      );
    };
  },
});
