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
/** Delay loading feedback so short image decodes do not flash visible chrome. */
import { defineComponent, h, onBeforeUnmount, ref, watch } from 'vue';

/** Own the presentation delay independently of image resources and workflow state. */
export const CanvasLoadingIndicator = defineComponent({
  name: 'CanvasLoadingIndicator',
  props: { loading: { type: Boolean, required: true } },
  /** Cancel delayed feedback on completion and attachment retirement. */
  setup(props) {
    const visible = ref(false);
    let timer: ReturnType<typeof setTimeout> | null = null;
    /** Retire the delay together with its visible feedback. */
    const clear = (): void => {
      if (timer !== null) clearTimeout(timer);
      timer = null;
      visible.value = false;
    };
    watch(
      () => props.loading,
      (loading) => {
        clear();
        if (loading)
          timer = setTimeout(() => {
            timer = null;
            visible.value = true;
          }, 250);
      },
      { immediate: true },
    );
    onBeforeUnmount(clear);
    return () =>
      visible.value
        ? h(
            'div',
            {
              class: 'sc-output-loading',
              role: 'status',
              'aria-label': 'Loading workflow image',
            },
            [h('span', { class: 'sc-output-spinner', 'aria-hidden': 'true' })],
          )
        : null;
  },
});
