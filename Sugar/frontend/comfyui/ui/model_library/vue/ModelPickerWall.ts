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
  inject,
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
import { ModelBlankCard } from './ModelBlankCard.js';
import { visibleModelRows, MODEL_PICKER_WALL_PROFILE } from '../ModelPickerLayout.js';
import type { ModelWallProfile } from '../ModelPickerLayout.js';
import { layoutModelGallery } from '../ModelGalleryLayout.js';
import type { ModelUpdateOffer } from '../ModelUpdates.js';
import { ModelUpdateButton } from './ModelUpdateButton.js';
import { ModelGalleryTitle } from './ModelGalleryTitle.js';
import { shownModels } from './ShownModels.js';

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
    // The model under the pointer, by identity, so a republished catalog keeps it.
    const hovered = ref<string | null>(null);
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
    // Tell the model library which models are on screen once the wall has come to rest.
    const shown = inject(shownModels, null);
    let reported = '';
    let settling: ReturnType<typeof setTimeout> | null = null;
    onBeforeUnmount(() => {
      if (settling !== null) clearTimeout(settling);
    });
    watch(
      () => [rows.value, scrollTop.value, height.value, props.items],
      () => {
        if (!shown) return;
        if (settling !== null) clearTimeout(settling);
        settling = setTimeout(() => {
          settling = null;
          const items = visibleModelRows(rows.value, scrollTop.value, height.value).flatMap((row) =>
            row.tiles.flatMap((tile) => props.items[tile.index] ?? []),
          );
          const key = items.map((item) => item.id).join(' ');
          if (key === reported) return;
          reported = key;
          shown(items);
        }, 150);
      },
      { immediate: true },
    );
    // The wall moves to the active model when the picker opens and when another model becomes
    // active. A republished catalog that leaves that model active leaves the scroll where the
    // person put it.
    watch(
      () => props.items[props.active]?.id,
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
              // A blurred image sharpens only under the pointer, and only when the library
              // sent its sharp files; moving through the wall with the keyboard reveals nothing.
              const revealed = hovered.value === item.id && !!item.image?.reveal.length;
              const image = chooseModelThumbnail(
                revealed ? { ...item, thumbnails: item.image?.reveal ?? [] } : item,
                'standard',
                Math.max(tile.width, tile.height) * devicePixelRatio,
              );
              const state = item.image?.state ?? 'none';
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
                  onPointerenter: () => {
                    hovered.value = item.id;
                    emit('highlight', tile.index);
                  },
                  onPointerleave: () => {
                    if (hovered.value === item.id) hovered.value = null;
                  },
                  onClick: () => emit('activate', item.value),
                  onContextmenu: (event: MouseEvent) => {
                    event.preventDefault();
                    event.stopPropagation();
                    emit('context', item, event);
                  },
                },
                [
                  image
                    ? h(ModelCatalogImage, { image, fallback: true })
                    : h(ModelBlankCard, { state: state === 'ready' ? 'none' : state }),
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
