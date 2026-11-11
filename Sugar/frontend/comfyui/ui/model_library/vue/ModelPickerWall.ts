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
/** Present a virtualized Substitute-style model image wall using Vue. */
import {
  computed,
  defineComponent,
  h,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  nextTick,
} from 'vue';
import type { PropType } from 'vue';
import { chooseModelThumbnail } from '../ModelCatalog.js';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { ModelCatalogImage } from './ModelCatalogImage.js';
import { visibleModelRows, MODEL_PICKER_WALL_PROFILE } from '../ModelPickerLayout.js';
import type { ModelWallProfile } from '../ModelPickerLayout.js';
import { layoutModelGallery } from '../ModelGalleryLayout.js';
import type { ModelUpdateOffer } from '../ModelUpdates.js';
import { ModelUpdateButton } from './ModelUpdateButton.js';
import { ModelGalleryTitle } from './ModelGalleryTitle.js';

/** Keep image decoding and mounted DOM bounded to the wall's visible rows. */
export const ModelPickerWall = defineComponent({
  name: 'SugarCubesModelPickerWall',
  props: {
    items: { type: Array as PropType<readonly ModelCatalogItem[]>, required: true },
    selected: { type: String, required: true },
    active: { type: Number, required: true },
    optionPrefix: { type: String, required: true },
    updateOffers: { type: Array as PropType<readonly ModelUpdateOffer[]>, default: () => [] },
    profile: {
      type: Object as PropType<ModelWallProfile>,
      default: () => MODEL_PICKER_WALL_PROFILE,
    },
  },
  emits: {
    activate: (_value: string) => true,
    highlight: (_index: number) => true,
    context: (_item: ModelCatalogItem, _event: MouseEvent) => true,
    geometry: (_width: number) => true,
    family: (_item: ModelCatalogItem) => true,
    updateContext: (_item: ModelCatalogItem, _event: MouseEvent) => true,
  },
  setup(props, { emit }) {
    const viewport = ref<HTMLElement | null>(null);
    const width = ref(540);
    const height = ref(580);
    const scrollTop = ref(0);
    const rows = computed(() => layoutModelGallery(props.items, width.value, props.profile));
    let observer: ResizeObserver | null = null;
    onMounted(() => {
      const element = viewport.value;
      if (!element) return;
      const measure = (): void => {
        if (element.clientWidth > 0) {
          width.value = element.clientWidth;
          emit('geometry', width.value);
        }
        if (element.clientHeight > 0) height.value = element.clientHeight;
      };
      measure();
      observer = new ResizeObserver(measure);
      observer.observe(element);
    });
    onBeforeUnmount(() => observer?.disconnect());
    watch(
      () => [props.active, props.items],
      () => {
        void nextTick(() => {
          const element = viewport.value;
          const row = rows.value.find((candidate) =>
            candidate.tiles.some((tile) => tile.index === props.active),
          );
          if (!element || !row) return;
          if (row.top < element.scrollTop) element.scrollTop = row.top;
          else if (row.top + row.height > element.scrollTop + height.value)
            element.scrollTop = Math.max(0, row.top + row.height - height.value);
          scrollTop.value = element.scrollTop;
        });
      },
      { immediate: true },
    );
    return () =>
      h(
        'div',
        {
          ref: viewport,
          class: 'sc-model-wall-viewport',
          'data-sc-scrollbar-edge': 'parent',
          onScroll: (event: Event) => {
            if (event.target instanceof HTMLElement) scrollTop.value = event.target.scrollTop;
          },
        },
        h(
          'div',
          {
            class: 'sc-model-wall',
            role: 'listbox',
            'aria-label': 'Models',
            style: {
              height: `${String((rows.value.at(-1)?.top ?? 0) + (rows.value.at(-1)?.height ?? 0))}px`,
            },
          },
          visibleModelRows(rows.value, scrollTop.value, height.value).flatMap((row) =>
            row.tiles.map((tile) => {
              const item = props.items[tile.index];
              if (!item) return null;
              const image = chooseModelThumbnail(
                item,
                'standard',
                Math.max(tile.width, tile.height) * devicePixelRatio,
              );
              return h(
                'div',
                {
                  key: item.id,
                  id: `${props.optionPrefix}-option-${String(tile.index)}`,
                  role: 'option',
                  tabindex: -1,
                  class: [
                    'sc-model-tile',
                    {
                      'sc-model-current': item.value === props.selected,
                      'sc-model-active': tile.index === props.active,
                    },
                  ],
                  'aria-selected': item.value === props.selected,
                  'aria-label': [item.title, item.version].filter(Boolean).join(', '),
                  title: item.relativePath,
                  'data-model-value': item.value,
                  style: {
                    left: `${String(tile.left)}px`,
                    top: `${String(tile.top)}px`,
                    width: `${String(tile.width)}px`,
                    height: `${String(tile.height)}px`,
                  },
                  onPointerenter: () => emit('highlight', tile.index),
                  onClick: () => emit('activate', item.value),
                  onContextmenu: (event: MouseEvent) => {
                    event.preventDefault();
                    event.stopPropagation();
                    emit('context', item, event);
                  },
                },
                [
                  image
                    ? h(ModelCatalogImage, { image, fallback: true, lazy: true })
                    : h('span', { class: 'sc-model-no-image' }, 'No image'),
                  h('span', { class: 'sc-model-caption' }, [
                    h(ModelGalleryTitle, {
                      title: item.title,
                      active: tile.index === props.active,
                    }),
                    item.version ? h('span', { class: 'sc-model-version' }, item.version) : null,
                  ]),
                  props.updateOffers.some((offer) => offer.modelId === item.id)
                    ? h(ModelUpdateButton, {
                        onOpen: () => emit('family', item),
                        onContext: (event: MouseEvent) => emit('updateContext', item, event),
                      })
                    : null,
                ],
              );
            }),
          ),
        ),
      );
  },
});
