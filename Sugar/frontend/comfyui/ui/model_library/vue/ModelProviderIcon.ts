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
/** Render licensed provider brands with a Microsoft Fluent fallback for unknown providers. */
import { defineComponent, h } from 'vue';

const brands: Readonly<Record<string, string>> = {
  civitai: new URL('../../../../model-providers/civitai.svg', import.meta.url).href,
  openmodeldb: new URL('../../../../model-providers/openmodeldb.ico', import.meta.url).href,
};
const globe = new URL('../../../../icons/model/globe.svg', import.meta.url).href;

/** Create a literal provider asset for the universal menu's icon slot. */
export function createModelProviderIcon(document: Document, provider: string): HTMLElement {
  const icon = document.createElement('img');
  icon.className = 'sc-model-provider-icon';
  icon.src = brands[provider] ?? globe;
  icon.alt = '';
  icon.draggable = false;
  return icon;
}

/** Keep source-menu and portrait badges under one asset-selection owner. */
export const ModelProviderIcon = defineComponent({
  name: 'SugarCubesModelProviderIcon',
  props: { provider: { type: String, required: true } },
  setup(props) {
    return () => {
      const source = brands[props.provider];
      return source
        ? h('img', { class: 'sc-model-provider-icon', src: source, alt: '', draggable: false })
        : h('span', {
            class: 'sc-model-provider-icon',
            'aria-hidden': 'true',
            style: { '--sc-model-provider-icon': `url("${globe}")` },
          });
    };
  },
});
