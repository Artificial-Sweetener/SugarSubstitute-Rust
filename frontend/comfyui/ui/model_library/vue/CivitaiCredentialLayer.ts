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
/** Render Substitute's contained explicit-key prompt without browser-native dialogs. */

import { defineComponent, h, nextTick, onMounted, ref } from 'vue';
import type { PropType } from 'vue';
import type { ModelDiscoveryViewActions } from '../ModelDiscoveryView.js';
const globeIcon = new URL('../../../../icons/model/globe.svg', import.meta.url).href;

/** Keep typed key input and focus inside the discovery dialog's own credential layer. */
export const CivitaiCredentialLayer = defineComponent({
  name: 'SugarCubesCivitaiCredentialLayer',
  props: {
    busy: { type: Boolean, required: true },
    error: { type: String, required: true },
    actions: { type: Object as PropType<ModelDiscoveryViewActions>, required: true },
  },
  setup(props) {
    const key = ref('');
    const visible = ref(false);
    const input = ref<HTMLInputElement | null>(null);
    const panel = ref<HTMLElement | null>(null);
    onMounted(() => {
      void nextTick(() => input.value?.focus());
    });
    /** Trap Tab locally and let Escape cancel only the contained prompt. */
    const keyboard = (event: KeyboardEvent): void => {
      event.stopPropagation();
      if (event.key === 'Escape') {
        event.preventDefault();
        if (!props.busy) props.actions.cancelCredential();
      }
      if (event.key !== 'Tab') return;
      const controls = [
        ...(panel.value?.querySelectorAll<HTMLElement>(
          'a[href],button:not(:disabled),input:not(:disabled)',
        ) ?? []),
      ];
      const first = controls[0],
        last = controls.at(-1);
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    return () =>
      h('div', { class: 'sc-model-credential-layer', onKeydown: keyboard }, [
        h(
          'form',
          {
            ref: panel,
            class: 'sc-model-credential-panel sugarcubes-modal__panel',
            role: 'dialog',
            'aria-modal': 'true',
            'aria-label': 'CivitAI API key',
            onSubmit: (event: Event) => {
              event.preventDefault();
              if (!props.busy && key.value.trim()) {
                const candidate = key.value;
                key.value = '';
                props.actions.saveCredential(candidate);
              }
            },
          },
          [
            h(
              'h2',
              { class: 'sugarcubes-modal__panel-title' },
              'This model needs a CivitAI API key',
            ),
            h(
              'p',
              'Sign in to CivitAI or create an account.\nThen create an API key in Account Settings and paste it here.',
            ),
            h(
              'a',
              {
                class: 'p-button p-button-secondary',
                href: 'https://civitai.com/user/account',
                target: '_blank',
                rel: 'noopener noreferrer',
              },
              [
                h('span', {
                  class: 'sc-model-action-icon',
                  'aria-hidden': 'true',
                  style: { '--sc-model-action-icon': `url("${globeIcon}")` },
                }),
                'Open CivitAI account settings',
              ],
            ),
            h('div', { class: 'sc-model-credential-input sugarcubes-modal__input-group' }, [
              h('input', {
                ref: input,
                type: visible.value ? 'text' : 'password',
                placeholder: 'Paste CivitAI API key',
                'aria-label': 'Paste CivitAI API key',
                autocomplete: 'off',
                spellcheck: false,
                disabled: props.busy,
                value: key.value,
                onInput: (event: Event) => {
                  if (event.target instanceof HTMLInputElement) key.value = event.target.value;
                },
              }),
              h(
                'button',
                {
                  type: 'button',
                  'aria-label': visible.value ? 'Hide API key' : 'Show API key',
                  disabled: props.busy,
                  onClick: () => {
                    visible.value = !visible.value;
                    input.value?.focus();
                  },
                },
                [h('span', { class: 'sc-model-credential-eye', 'aria-hidden': 'true' })],
              ),
            ]),
            props.error
              ? h('p', { role: 'alert', class: 'sc-model-credential-error' }, props.error)
              : null,
            h('div', { class: 'sc-model-credential-actions' }, [
              h(
                'button',
                {
                  type: 'button',
                  class: 'p-button p-button-secondary',
                  disabled: props.busy,
                  onClick: props.actions.cancelCredential,
                },
                'Cancel',
              ),
              h(
                'button',
                {
                  type: 'button',
                  class: 'p-button p-button-secondary',
                  disabled: props.busy,
                  onClick: props.actions.publicOnly,
                },
                "Show models that don't require a key",
              ),
              h(
                'button',
                {
                  type: 'submit',
                  class: 'p-button p-button-primary',
                  disabled: props.busy || !key.value.trim(),
                },
                'Save and continue',
              ),
            ]),
          ],
        ),
      ]);
  },
});
