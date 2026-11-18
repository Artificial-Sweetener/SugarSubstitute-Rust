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
/** Present the picker popup on its own, with its search box, for choosing one model at a point. */
import { defineComponent, h, nextTick, onBeforeUnmount, onMounted, ref, useId } from 'vue';
import type { PropType } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { modelPickerPlacement } from '../ModelPickerPlacement.js';
import type { ModelPickerRectangle } from '../ModelPickerPlacement.js';
import { useModelBrowsing } from './ModelBrowsing.js';
import { ModelPickerPopupContent } from './ModelPickerPopupContent.js';

/** Browse and choose without a workflow field: the caller receives the model or a dismissal. */
export const ModelChooser = defineComponent({
  name: 'SugarCubesModelChooser',
  props: {
    items: { type: Array as PropType<readonly ModelCatalogItem[]>, required: true },
    /** Names the popup and its search box, e.g. "Search LoRA". */
    label: { type: String, required: true },
    /** The viewport point or rectangle the popup opens beside. */
    anchor: { type: Object as PropType<ModelPickerRectangle>, required: true },
  },
  emits: {
    choose: (_item: ModelCatalogItem) => true,
    dismiss: () => true,
  },
  setup(props, { emit }) {
    const browsing = useModelBrowsing(() => props.items);
    const input = ref<HTMLInputElement | null>(null);
    const popup = ref<HTMLElement | null>(null);
    const popupId = useId();
    const geometry = modelPickerPlacement(
      { left: 0, top: 0, width: innerWidth, height: innerHeight },
      props.anchor,
    );
    const placement = {
      left: `${String(geometry.left)}px`,
      top: `${String(geometry.top)}px`,
      width: `${String(geometry.width)}px`,
      height: `${String(geometry.height)}px`,
    };

    /** Choose the model with this value when it is one of the models shown. */
    const choose = (value: string): void => {
      const item = browsing.visible.value.find((candidate) => candidate.value === value);
      if (item) emit('choose', item);
    };
    /** A press anywhere else leaves without choosing. */
    const outside = (event: Event): void => {
      if (event.target instanceof Node && !popup.value?.contains(event.target)) emit('dismiss');
    };
    /** Route keys to browsing so neither the prompt nor the graph handles them. */
    const key = (event: KeyboardEvent): void => {
      event.stopPropagation();
      if (event.key === 'Escape') {
        event.preventDefault();
        emit('dismiss');
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const item = browsing.visible.value[browsing.active.value];
        if (item) emit('choose', item);
      } else if (event.key === 'Tab') {
        // Focus stays in the search box; Tab only ever extends the search.
        event.preventDefault();
        browsing.complete();
      } else if (browsing.move(event.key)) event.preventDefault();
    };
    onMounted(() => {
      document.addEventListener('pointerdown', outside, true);
      void nextTick(() => input.value?.focus());
    });
    onBeforeUnmount(() => document.removeEventListener('pointerdown', outside, true));

    return () =>
      h(
        'div',
        {
          ref: popup,
          id: popupId,
          class: 'sc-model-popup',
          role: 'dialog',
          'aria-label': props.label,
          style: placement,
          onPointerdown: (event: Event) => event.stopPropagation(),
          onWheel: (event: Event) => event.stopPropagation(),
        },
        [
          h('div', { class: 'sc-model-field sc-model-field-open sc-model-popup-search' }, [
            h('input', {
              ref: input,
              value: browsing.query.value,
              placeholder: props.label,
              role: 'combobox',
              'aria-label': props.label,
              'aria-expanded': true,
              'aria-controls': popupId,
              'aria-activedescendant': browsing.visible.value[browsing.active.value]
                ? `${popupId}-option-${String(browsing.active.value)}`
                : undefined,
              'aria-autocomplete': 'both',
              spellcheck: false,
              onKeydown: key,
              onInput: (event: Event) => {
                if (event.target instanceof HTMLInputElement) browsing.search(event.target.value);
              },
            }),
            browsing.query.value && browsing.completion.value
              ? h('span', { class: 'sc-model-completion', 'aria-hidden': true }, [
                  h('span', { class: 'sc-model-completion-prefix' }, browsing.query.value),
                  browsing.completion.value.suffix,
                ])
              : null,
          ]),
          h(ModelPickerPopupContent, {
            items: browsing.navigable.value,
            visible: browsing.visible.value,
            folder: browsing.folder.value,
            selected: '',
            active: browsing.active.value,
            optionPrefix: popupId,
            onNavigate: (path: string) => {
              browsing.open(path);
              input.value?.focus();
            },
            onSelect: choose,
            onGeometry: (width: number) => {
              browsing.wallWidth.value = width;
            },
          }),
        ],
      );
  },
});
