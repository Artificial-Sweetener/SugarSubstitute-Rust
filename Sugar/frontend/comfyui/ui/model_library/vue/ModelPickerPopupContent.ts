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
/** Own the model wall and compact folder navigation without replacing browsing with detail pages. */
import { defineComponent, h } from 'vue';
import type { PropType } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { modelChoiceFolders } from '../ModelPickerChoices.js';
import { ModelPickerWall } from './ModelPickerWall.js';
import type { ModelUpdateOffer } from '../ModelUpdates.js';
import { ModelPickerRouteRow } from './ModelPickerRouteRow.js';

/** Keep navigation and context actions separate from a committed workflow choice. */
export const ModelPickerPopupContent = defineComponent({
  name: 'SugarCubesModelPickerPopupContent',
  props: {
    items: { type: Array as PropType<readonly ModelCatalogItem[]>, required: true },
    visible: { type: Array as PropType<readonly ModelCatalogItem[]>, required: true },
    folder: { type: String, required: true },
    selected: { type: String, required: true },
    active: { type: Number, required: true },
    optionPrefix: { type: String, required: true },
    updateOffers: { type: Array as PropType<readonly ModelUpdateOffer[]>, default: () => [] },
    compatibilityLabel: { type: String, default: '' },
    unfiltered: { type: Boolean, default: false },
  },
  emits: {
    toggleFilter: () => true,
    navigate: (_path: string) => true,
    select: (_value: string) => true,
    highlight: (_index: number) => true,
    geometry: (_width: number) => true,
    context: (_item: ModelCatalogItem, _event: MouseEvent) => true,
    family: (_item: ModelCatalogItem) => true,
    updateContext: (_item: ModelCatalogItem, _event: MouseEvent) => true,
  },
  setup(props, { emit }) {
    /** Navigate one folder without committing a node's model choice. */
    const route = (label: string, path: string, child = false) =>
      h(
        'button',
        {
          type: 'button',
          class: child ? 'sc-model-child-folder' : undefined,
          onClick: () => emit('navigate', path),
        },
        label,
      );
    /** Reserve the longer filter label so toggling keeps the icon and button edges stationary. */
    const filter = () =>
      props.compatibilityLabel
        ? h(
            'button',
            {
              type: 'button',
              class: 'sc-model-filter-toggle',
              'data-width-label': `Show only ${props.compatibilityLabel}`,
              'aria-label': props.unfiltered ? `Show only ${props.compatibilityLabel}` : 'Show all',
              'aria-pressed': props.unfiltered,
              title: props.unfiltered ? `Show only ${props.compatibilityLabel}` : 'Show all models',
              onClick: () => emit('toggleFilter'),
            },
            [
              h('span', { class: 'sc-model-filter-icon', 'aria-hidden': true }),
              h(
                'span',
                { class: 'sc-model-filter-label' },
                props.unfiltered ? `Show only ${props.compatibilityLabel}` : 'Show all',
              ),
            ],
          )
        : null;
    return () => {
      const children = modelChoiceFolders(props.items, props.folder);
      return [
        props.folder || children.length || props.compatibilityLabel
          ? h('nav', { class: 'sc-model-routes', 'aria-label': 'Model folders' }, [
              ...(props.folder
                ? [
                    h(ModelPickerRouteRow, { rowClass: 'sc-model-breadcrumbs' }, () => [
                      route('All', ''),
                      ...props.folder
                        .split('/')
                        .filter(Boolean)
                        .flatMap((part, index, parts) => [
                          h(
                            'span',
                            { class: 'sc-model-route-separator', 'aria-hidden': true },
                            '›',
                          ),
                          route(part, parts.slice(0, index + 1).join('/')),
                        ]),
                      children.length ? null : filter(),
                    ]),
                  ]
                : []),
              ...(children.length || (!props.folder && props.compatibilityLabel)
                ? [
                    h(ModelPickerRouteRow, { rowClass: 'sc-model-folder-strip' }, () => [
                      ...children.map((path) => route(path.split('/').at(-1) ?? path, path, true)),
                      filter(),
                    ]),
                  ]
                : []),
            ])
          : null,
        props.visible.length
          ? h(ModelPickerWall, {
              items: props.visible,
              selected: props.selected,
              active: props.active,
              optionPrefix: props.optionPrefix,
              updateOffers: props.updateOffers,
              onActivate: (value: string) => emit('select', value),
              onHighlight: (index: number) => emit('highlight', index),
              onGeometry: (width: number) => emit('geometry', width),
              onContext: (item: ModelCatalogItem, event: MouseEvent) =>
                emit('context', item, event),
              onFamily: (item: ModelCatalogItem) => emit('family', item),
              onUpdateContext: (item: ModelCatalogItem, event: MouseEvent) =>
                emit('updateContext', item, event),
            })
          : h('div', { class: 'sc-model-empty' }, 'No matching models'),
      ];
    };
  },
});
