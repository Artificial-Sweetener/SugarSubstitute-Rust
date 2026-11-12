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
/** Measure each route strip independently so horizontal scrolling does not enlarge every row. */
import { defineComponent, h, onBeforeUnmount, onMounted, onUpdated, ref } from 'vue';

/** Reserve the floating scrollbar hit area only while this route row actually overflows. */
export const ModelPickerRouteRow = defineComponent({
  name: 'SugarCubesModelPickerRouteRow',
  props: { rowClass: { type: String, required: true } },
  setup(props, { slots }) {
    const element = ref<HTMLElement | null>(null);
    const overflowing = ref(false);
    let observer: ResizeObserver | null = null;
    /** Follow width and rendered children without coupling navigation to scrollbar infrastructure. */
    const measure = (): void => {
      overflowing.value = !!element.value && element.value.scrollWidth > element.value.clientWidth;
    };
    onMounted(() => {
      observer = new ResizeObserver(measure);
      if (element.value) observer.observe(element.value);
      measure();
    });
    onUpdated(measure);
    onBeforeUnmount(() => observer?.disconnect());
    return () =>
      h(
        'div',
        {
          ref: element,
          class: [props.rowClass, { 'sc-model-route-overflow': overflowing.value }],
        },
        slots.default?.(),
      );
  },
});
