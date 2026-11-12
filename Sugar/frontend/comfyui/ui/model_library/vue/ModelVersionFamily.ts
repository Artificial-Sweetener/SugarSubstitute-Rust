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
/** Render model version choices independently of dialog and transfer lifetimes. */
import { defineComponent, h, ref, watch, nextTick } from 'vue';
import type { PropType } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { chooseModelThumbnail } from '../ModelCatalog.js';
import type { ModelRelease } from '../ModelReleases.js';
import { modelArtifactSize } from '../ModelArtifactPresentation.js';
import { ModelRecommendationPreview } from './ModelRecommendationPreview.js';
import { ModelBusyRing } from './ModelBusyRing.js';
import { ModelKeyRequiredBadge } from './ModelKeyRequiredBadge.js';
import { scrollWheelDistance } from '../../surface/ScrollWheelDistance.js';

/** Present exclusive acquisition choices while retaining full-width horizontal timeline scrolling. */
export const ModelVersionFamily = defineComponent({
  name: 'SugarCubesModelVersionFamily',
  props: {
    item: { type: Object as PropType<ModelCatalogItem>, required: true },
    versions: { type: Array as PropType<readonly ModelRelease[]>, required: true },
    selected: { type: String, required: true },
    busy: { type: Boolean, required: true },
  },
  emits: { select: (_version: ModelRelease) => true },
  setup(props, { emit }) {
    const timeline = ref<HTMLElement | null>(null);
    /** Own both wheel axes before nested preview handlers while preserving browser zoom. */
    const scroll = (event: WheelEvent): void => {
      if (event.ctrlKey) return;
      event.stopPropagation();
      const host = timeline.value;
      if (!host || host.scrollWidth <= host.clientWidth) return;
      const distance = scrollWheelDistance(event, 'horizontal', host.clientWidth);
      if (!distance) return;
      event.preventDefault();
      host.scrollLeft = Math.min(
        host.scrollWidth - host.clientWidth,
        Math.max(0, host.scrollLeft + distance),
      );
    };
    watch(
      () => props.versions,
      () => {
        void nextTick(() => {
          const host = timeline.value;
          const current = host?.querySelector<HTMLElement>('[data-in-use=true]');
          if (host && current)
            host.scrollLeft = Math.max(
              0,
              current.offsetLeft - host.clientWidth / 2 + current.offsetWidth / 2,
            );
        });
      },
      { immediate: true },
    );
    return () =>
      h('section', { class: 'sc-model-version-family', 'aria-label': 'Model version family' }, [
        h(
          'div',
          {
            ref: timeline,
            class: 'sc-model-timeline',
            'aria-label': 'Model versions',
            onWheelCapture: scroll,
          },
          props.versions.length
            ? props.versions.map((version) => {
                const inUse = version.inUse ?? version.versionId === props.item.providerVersionId;
                const onDisk = version.onDisk;
                const selected = props.selected === version.versionId;
                const access = version.files[0]?.access;
                const availability = onDisk
                  ? 'On disk'
                  : access === 'api_key_required'
                    ? 'API key required'
                    : access === 'unknown'
                      ? 'Access unavailable'
                      : 'Download available';
                const image = inUse
                  ? (chooseModelThumbnail(props.item, 'standard', 512)?.url ?? version.previewUrl)
                  : version.previewUrl;
                return h(
                  'button',
                  {
                    type: 'button',
                    class: ['sc-model-version-card', { 'sc-model-version-selected': selected }],
                    'aria-label': `${version.name}, ${inUse ? 'In use' : availability}`,
                    'aria-pressed': selected,
                    'data-in-use': inUse,
                    disabled:
                      props.busy ||
                      (onDisk
                        ? !version.installedFileName
                        : !version.files.length || version.files[0]?.access === 'unknown'),
                    onClick: () => emit('select', version),
                  },
                  [
                    h('span', { class: 'sc-model-version-portrait' }, [
                      h(ModelRecommendationPreview, { url: image }),
                      !onDisk && access === 'api_key_required'
                        ? h(ModelKeyRequiredBadge, { providerName: 'CivitAI' })
                        : null,
                      h('span', { class: 'sc-model-version-name' }, version.name),
                      h('span', { class: 'sc-model-version-check', 'aria-hidden': true }, [
                        selected
                          ? h(
                              'svg',
                              { viewBox: '0 0 16 16', 'aria-hidden': true, focusable: 'false' },
                              [
                                h('path', {
                                  d: 'M3 8 6.5 11.5 13 4.5',
                                  fill: 'none',
                                  stroke: 'currentColor',
                                  'stroke-width': 2,
                                  'stroke-linecap': 'round',
                                  'stroke-linejoin': 'round',
                                }),
                              ],
                            )
                          : null,
                      ]),
                    ]),
                    h(
                      'span',
                      { class: 'sc-model-version-state' },
                      onDisk ? 'On disk' : modelArtifactSize(version.files[0]?.sizeBytes ?? 0),
                    ),
                  ],
                );
              })
            : [
                h(
                  'div',
                  { class: 'sc-model-version-loading', role: 'status' },
                  props.busy
                    ? [h(ModelBusyRing, { size: 22 }), 'Loading model versions...']
                    : 'No model versions available.',
                ),
              ],
        ),
      ]);
  },
});
