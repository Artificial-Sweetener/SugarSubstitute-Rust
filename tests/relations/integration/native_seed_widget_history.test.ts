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
/** Prove Comfy's cached local mode follows external history while native edits retain control identity. */
import { jest } from '@jest/globals';
import { createApp, defineComponent, h, nextTick, ref, render, watch } from 'vue';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { OverrideGroup } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import type { RelationSeedMode } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';
import { readNativeWidgetMount } from '../../../frontend/comfyui/ui/surface/NativeWidgetMountBoundary.js';

/** Model the installed WidgetWithControl contract: initialize a private ref once and publish native changes. */
const nativeNumber = defineComponent({
  props: ['widget', 'modelValue'],
  setup(props) {
    const initial: unknown = props.widget;
    if (!isRecord(initial) || !isRecord(initial.controlWidget)) throw new Error('Missing control');
    const mode = ref<unknown>(initial.controlWidget.value);
    watch(mode, (value) => {
      const widget: unknown = props.widget;
      const control =
        isRecord(widget) && isRecord(widget.controlWidget) ? widget.controlWidget : {};
      if (typeof control.update === 'function') control.update(value);
    });
    return () =>
      h('div', [
        h('input', { role: 'spinbutton', value: String(props.modelValue) }),
        h(
          'button',
          {
            'data-testid': 'value-control',
            'data-mode': String(mode.value),
            onClick: () => {
              mode.value = 'randomize';
            },
          },
          String(mode.value),
        ),
      ]);
  },
});

jest.unstable_mockModule(
  '../../../frontend/comfyui/ui/surface/ComfyNativeWidgetRuntime.js',
  () => ({
    loadComfyNativeWidgetRuntime: async () => ({
      h,
      render,
      components: new Map([['WidgetInputNumber', nativeNumber]]),
    }),
  }),
);
const { NativeRelationWidgetRenderer } = await import(
  '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWidgetRenderer.js'
);

/** Mount the real adapter into an actual Vue context with an explicitly owned appearance source. */
async function fixture(fluent: boolean) {
  const appRoot = document.createElement('div');
  appRoot.id = 'vue-app';
  const root = document.createElement('div');
  document.body.append(appRoot, root);
  const app = createApp({ render: () => h('main') });
  app.mount(appRoot);
  const appearance = new AppearanceState();
  appearance.update(fluent, true);
  const domain = { kind: 'number' as const, minimum: 0, maximum: 1000, step: 1, integer: true };
  let group: OverrideGroup = {
    key: 'seed',
    label: 'Seed',
    domain,
    value: 42,
    enabled: true,
    seedMode: 'fixed',
    members: [
      {
        id: 'seed',
        nodeId: 'node',
        cubeId: null,
        name: 'seed',
        semantic: 'seed',
        label: 'Seed',
        value: 42,
        domain,
      },
    ],
  };
  const error = jest.fn();
  const renderer = new NativeRelationWidgetRenderer(document, error, undefined, () => appearance);
  const mount = renderer.mount(
    root,
    group,
    () => undefined,
    (mode) => update(mode, group.value),
  );
  /** Project native edits or externally restored history through the same mounted adapter. */
  function update(mode: RelationSeedMode, value: OverrideGroup['value']): void {
    group = { ...group, seedMode: mode, value };
    mount.update(group);
  }
  await Promise.resolve();
  await Promise.resolve();
  await nextTick();
  return {
    root,
    appearance,
    update,
    error,
    dispose: () => {
      mount.dispose();
      app.unmount();
      appearance.dispose();
      document.body.replaceChildren();
    },
  };
}

test('retains native control identity for native mode edits and restores focused value and mode after undo', async () => {
  const f = await fixture(false);
  const originalInput = f.root.querySelector<HTMLInputElement>('input');
  const originalButton = f.root.querySelector<HTMLButtonElement>('button');
  originalButton?.click();
  await nextTick();
  expect(f.root.querySelector('button')).toBe(originalButton);
  expect(f.root.querySelector('input')).toBe(originalInput);
  expect(originalButton?.dataset.mode).toBe('randomize');
  originalInput?.focus();
  f.update('fixed', 77);
  await nextTick();
  expect(f.root.querySelector<HTMLButtonElement>('button')?.dataset.mode).toBe('fixed');
  expect(f.root.querySelector<HTMLInputElement>('input')?.value).toBe('77');
  expect(document.activeElement).toBe(f.root.querySelector('input'));
  expect(f.error).not.toHaveBeenCalled();
  f.dispose();
});

test('keeps Fluent field identity during mode edits and synchronizes cached native mode when returning to normal appearance', async () => {
  const f = await fixture(true);
  const originalInput = f.root.querySelector('input');
  readNativeWidgetMount(f.root)?.seed?.writeMode('randomize');
  await nextTick();
  expect(f.root.querySelector('input')).toBe(originalInput);
  f.appearance.update(false, true);
  await nextTick();
  expect(f.root.querySelector<HTMLButtonElement>('button')?.dataset.mode).toBe('randomize');
  expect(f.root.querySelector<HTMLInputElement>('input')?.value).toBe('42');
  expect(f.error).not.toHaveBeenCalled();
  f.dispose();
});
