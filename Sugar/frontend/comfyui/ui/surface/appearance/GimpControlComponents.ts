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
/** Render native input semantics in Vue custom elements with inherited GIMP materials. */
import { computed, defineComponent, h, useHost } from 'vue';
import { publishGimpInput } from './GimpInputEvents.js';

/** Render an actual GTK-style track and thumb beside the host-owned numeric spinner. */
export const GimpSliderComponent = defineComponent({
  name: 'GimpSlider',
  inheritAttrs: false,
  props: {
    min: { default: 0 },
    max: { default: 1 },
    step: { default: 0.01 },
    value: { default: 0 },
    disabled: Boolean,
  },
  /** Publish native range edits through the same canonical value binding as the spinner. */
  setup(props, { attrs }) {
    const host = useHost();
    const fraction = computed(() => {
      const extent = Number(props.max) - Number(props.min);
      return extent > 0
        ? Math.max(0, Math.min(1, (Number(props.value) - Number(props.min)) / extent))
        : 0;
    });
    return () =>
      h('input', {
        type: 'range',
        class: 'scale-slider',
        ...props,
        'aria-label': attrs.ariaLabel ?? 'Value',
        style: { '--sc-gimp-slider-fraction': `${String(fraction.value * 100)}%` },
        onInput: (event: Event) => publishGimpInput(host, event, 'value'),
        onChange: (event: Event) => publishGimpInput(host, event, 'value'),
      });
  },
});

/** Let shared consumers supply icons and literal text through a stable native slot. */
export const GimpButtonComponent = defineComponent({
  name: 'GimpButton',
  inheritAttrs: false,
  props: { disabled: Boolean },
  /** Keep actual button activation and disabled handling in the browser. */
  setup(props, { attrs }) {
    return () =>
      h(
        'button',
        {
          type: 'button',
          disabled: props.disabled,
          'aria-label': attrs.ariaLabel,
          // The tab host owns sequential focus; its native button still supports pointer activation.
          tabindex: attrs.role === 'tab' ? -1 : undefined,
        },
        [h('slot')],
      );
  },
});

/** Retain accessible native checkbox behavior while projecting checked state to the adapter. */
export const GimpCheckboxComponent = defineComponent({
  name: 'GimpCheckbox',
  inheritAttrs: false,
  props: { checked: Boolean, disabled: Boolean },
  /** Publish one change, allowing the host binding to own history and canonical values. */
  setup(props, { attrs }) {
    const host = useHost();
    return () =>
      h('input', {
        type: 'checkbox',
        checked: props.checked,
        disabled: props.disabled,
        'aria-label': attrs.ariaLabel,
        onChange: (event: Event) => publishGimpInput(host, event, 'checked'),
      });
  },
});

/** Give imperative and declarative finder views the same Vue search entry. */
export const GimpSearchComponent = defineComponent({
  name: 'GimpSearch',
  inheritAttrs: false,
  props: { value: { default: '' }, placeholder: { default: '' }, disabled: Boolean },
  /** Preserve the real input across query updates rather than replacing its selection. */
  setup(props, { attrs }) {
    const host = useHost();
    return () =>
      h('input', {
        type: 'search',
        ...props,
        'aria-label': attrs.ariaLabel ?? 'Search masonry',
        onInput: (event: Event) => publishGimpInput(host, event, 'value'),
      });
  },
});

/** Keep external CodeMirror and workflow-tab children in slots owned by their original lifecycles. */
export const GimpFrameComponent = defineComponent({
  name: 'GimpFrame',
  /** Render chrome only, never a second editor or tab interaction model. */
  setup() {
    return () => h('div', { class: 'frame' }, [h('slot')]);
  },
});
