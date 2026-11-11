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
/** Present Substitute's exclusive portrait choices and their distinct provider-source menus. */

import { defineComponent, h, shallowRef } from 'vue';
import type { PropType } from 'vue';
import type { ModelDiscoveryViewState, ModelDiscoveryViewActions } from '../ModelDiscoveryView.js';
import type { ModelDiscoverySuggestion } from '../ModelDiscovery.js';
import type { ModelContextEntry } from '../ModelContextMenu.js';
import { ModelContextMenuView } from './ModelContextMenuView.js';
import { CivitaiCredentialLayer } from './CivitaiCredentialLayer.js';
import { ModelDiscoveryPortrait } from './ModelDiscoveryPortrait.js';
import { ModelProviderIcon } from './ModelProviderIcon.js';
import { ModelBusyRing } from './ModelBusyRing.js';
import { ModelKeyRequiredBadge } from './ModelKeyRequiredBadge.js';

/** Render supplied reviewed state while keeping selection and acquisition policy in the dialog owner. */
export const ModelDiscoveryGallery = defineComponent({
  name: 'SugarCubesModelDiscoveryGallery',
  props: {
    state: { type: Object as PropType<ModelDiscoveryViewState>, required: true },
    actions: { type: Object as PropType<ModelDiscoveryViewActions>, required: true },
  },
  setup(props) {
    const menu = shallowRef<{
      x: number;
      y: number;
      entries: readonly ModelContextEntry[];
      sources: ReadonlyMap<ModelContextEntry, string>;
      identity: string;
    } | null>(null);
    /** Preserve provider links and explicit alternate acquisition-source choices in one menu. */
    const showMenu = (item: ModelDiscoverySuggestion, event: MouseEvent): void => {
      event.preventDefault();
      event.stopPropagation();
      if (props.state.downloading || props.state.credential) return;
      const sources = new Map<ModelContextEntry, string>();
      const entries: ModelContextEntry[] = item.offers.map((offer) => ({
        label: `View ${item.modelName} on ${offer.providerName}`,
        enabled: true,
        command: null,
        url: offer.modelPageUrl,
        providerIcon: offer.providerId,
      }));
      if (item.offers.length > 1) {
        entries.push({ label: '', enabled: false, command: null, url: null, separator: true });
        for (const offer of item.offers) {
          const entry: ModelContextEntry = {
            label: `Download from ${offer.providerName}`,
            checked:
              (props.state.providers[item.identity] ?? item.offers[0]?.providerId) ===
              offer.providerId,
            providerIcon: offer.providerId,
            enabled: true,
            command: null,
            url: null,
          };
          sources.set(entry, offer.providerId);
          entries.push(entry);
        }
      }
      menu.value = {
        x: event.clientX,
        y: event.clientY,
        entries,
        sources,
        identity: item.identity,
      };
    };
    return () => {
      const state = props.state;
      const currentMenu = menu.value;
      return h(
        'div',
        {
          class: 'sc-model-discovery-view',
          onPointerdown: (event: Event) => event.stopPropagation(),
          onWheel: (event: Event) => event.stopPropagation(),
        },
        [
          h(
            'p',
            { class: 'sc-model-discovery-purpose' },
            state.artifactKind === 'upscale_models'
              ? 'Upscalers enlarge existing images and refine details.'
              : 'Image models create new images from your prompts.',
          ),
          state.loading
            ? h('div', { class: 'sc-model-discovery-loading', role: 'status' }, [
                h(ModelBusyRing),
                state.status,
              ])
            : null,
          !state.loading
            ? h(
                'div',
                {
                  class: 'sc-model-discovery-grid',
                  'aria-label': 'Compatible model suggestions',
                  inert: state.credential,
                },
                state.plan?.suggestions.map((item) => {
                  const offer =
                    item.offers.find(
                      (value) => value.providerId === state.providers[item.identity],
                    ) ?? item.offers[0];
                  if (!offer) return null;
                  const selected = state.selected === item.identity;
                  return h(
                    'div',
                    {
                      class: 'sc-model-discovery-card',
                      key: item.identity,
                      onContextmenu: (event: MouseEvent) => showMenu(item, event),
                    },
                    [
                      h(ModelDiscoveryPortrait, {
                        previewUrl: offer.previewUrl,
                        title: item.modelName,
                        selected,
                        disabled: state.downloading,
                        onSelect: () => {
                          menu.value = null;
                          props.actions.select(selected ? '' : item.identity);
                        },
                      }),
                      h(
                        'a',
                        {
                          class: 'sc-model-discovery-provider',
                          'aria-label': `View ${item.modelName} on ${offer.providerName}`,
                          href: state.downloading ? undefined : offer.modelPageUrl,
                          'aria-disabled': state.downloading,
                          tabindex: state.downloading ? -1 : 0,
                          target: '_blank',
                          rel: 'noopener noreferrer',
                        },
                        [h(ModelProviderIcon, { provider: offer.providerId })],
                      ),
                      offer.access === 'api_key_required'
                        ? h(ModelKeyRequiredBadge, { providerName: offer.providerName })
                        : null,
                    ],
                  );
                }),
              )
            : null,
          !state.loading && state.status
            ? h('div', { class: 'sc-model-discovery-status', role: 'status' }, [
                state.downloading ? h(ModelBusyRing) : null,
                state.status,
              ])
            : null,
          menu.value && !state.credential
            ? h(ModelContextMenuView, {
                entries: menu.value.entries,
                x: menu.value.x,
                y: menu.value.y,
                label: 'Model providers',
                onClose: () => {
                  menu.value = null;
                },
                onAction: (entry: ModelContextEntry) => {
                  const current = currentMenu;
                  const source = current?.sources.get(entry);
                  if (current && source) props.actions.provider(current.identity, source);
                  menu.value = null;
                },
              })
            : null,
          state.credential
            ? h(CivitaiCredentialLayer, {
                busy: state.credentialBusy,
                error: state.credentialError,
                actions: props.actions,
              })
            : null,
        ],
      );
    };
  },
});
