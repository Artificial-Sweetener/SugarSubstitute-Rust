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
/** Share the protected-download indicator across suggestion and version cards. */
import { defineComponent, h } from 'vue';

const keyIcon = new URL('../../../../icons/model/key.svg', import.meta.url).href;

/** Identify required credentials without replacing a card's size or installation status. */
export const ModelKeyRequiredBadge = defineComponent({
  name: 'SugarCubesModelKeyRequiredBadge',
  props: { providerName: { type: String, required: true } },
  setup(props) {
    return () =>
      h('span', {
        class: 'sc-model-key-required',
        title: `${props.providerName} API key required`,
        role: 'img',
        'aria-label': `${props.providerName} API key required`,
      });
  },
});

/** Keep the credential glyph's geometry and contrast consistent across portrait surfaces. */
export const modelKeyRequiredBadgeCss = `
.sc-model-key-required { position:absolute; width:24px; height:24px; border-radius:50%; border:1px solid #ffffff64; background:rgba(20,22,28,.88); pointer-events:none; }
.sc-model-key-required::after { content:''; position:absolute; inset:3px; background:#fff; mask:url("${keyIcon}") center/contain no-repeat; }
`;
