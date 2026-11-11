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
/** Prove workflow-owned transfers survive card replacement and completion with no mounted view. */
import { nextTick } from 'vue';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { ModelPickerSessions } from '../../../frontend/comfyui/ui/model_library/ModelPickerSessions.js';
import { readComfyModelFields } from '../../../frontend/comfyui/ui/model_library/ComfyModelFieldAdapter.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { nativeModelFieldFixture } from '../support/native_model_field.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';
import type { ModelTransferObserver } from '../../../frontend/comfyui/ui/model_library/ModelTransfer.js';

beforeAll(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
});

test('reattaches the same download to replacement cards across masonry and theme changes', async () => {
  const native = nativeModelFieldFixture();
  const app = { graph: native.graph };
  const registry = new ModelPickerSessions(document, () => native.snapshot, app);
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const session = registry.get(native.card, binding);
  let progress: ModelTransferObserver = () => undefined;
  let finish: (filename: string) => void = () => undefined;
  const operation = session.useDownload(
    async (publish) => {
      progress = publish;
      publish({ message: 'Downloading', percentage: 25 });
      return new Promise<string>((resolve) => {
        finish = resolve;
      });
    },
    { name: 'Accepted model', previewUrl: '/accepted-preview' },
  );
  let card = native.card;
  const mount = card.parentElement;
  try {
    for (const appearance of ['native', 'fluent', 'native', 'fluent']) {
      const nextCard = document.createElement('div');
      nextCard.className = 'lg-node';
      nextCard.dataset.nodeId = 'checkpoint';
      card.replaceWith(nextCard);
      nextCard.append(native.frame);
      if (!isRecord(mount)) throw new Error('Missing native mount.');
      const vnode = mount._vnode;
      const component = isRecord(vnode) && isRecord(vnode.component) ? vnode.component : null;
      if (!component || !isRecord(component.subTree)) throw new Error('Missing node presentation.');
      component.subTree.el = nextCard;
      registry.reconcile();
      const next = readComfyModelFields(nextCard, native.snapshot)[0];
      if (!next) throw new Error('Missing replacement field.');
      expect(registry.get(nextCard, next)).toBe(session);
      const root = document.createElement('div');
      root.dataset.scModelAppearance = appearance;
      nextCard.append(root);
      const picker = mountModelPicker(
        root,
        next,
        modelPickerChoices([], next.kind, next.options),
        null,
        null,
        null,
        null,
        null,
        null,
        session,
      );
      await nextTick();
      expect(root.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('25');
      expect(root.querySelector('input')?.value).toBe('Downloading Accepted model');
      expect(root.querySelector('.sc-model-transfer-thumbnail')?.getAttribute('src')).toBe(
        '/accepted-preview',
      );
      progress({ message: 'Downloading', percentage: 60 });
      await nextTick();
      expect(root.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('60');
      progress({ message: 'Downloading', percentage: 25 });
      picker.dispose();
      root.remove();
      card = nextCard;
    }
    native.props.widget.options = { values: ['downloaded.safetensors'] };
    finish('downloaded.safetensors');
    await operation;
    expect(native.props.modelValue).toBe('downloaded.safetensors');
    expect(session.read()).toBeNull();
  } finally {
    registry.clear();
    mount?.remove();
  }
});

test('commits to the workflow while its cards are absent and cancels only when the node is deleted', async () => {
  const native = nativeModelFieldFixture();
  const mount = native.card.parentElement;
  const registry = new ModelPickerSessions(document, () => native.snapshot, {
    graph: native.graph,
  });
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const session = registry.get(native.card, binding);
  let finish: (value: string) => void = () => undefined;
  const signal: { value: AbortSignal | null } = { value: null };
  const operation = session.useDownload(
    async (_publish, currentSignal) => {
      signal.value = currentSignal;
      return new Promise<string>((resolve) => {
        finish = resolve;
      });
    },
    { name: 'Accepted model', previewUrl: null },
  );
  native.card.remove();
  registry.reconcile();
  expect(signal.value?.aborted).toBe(false);
  native.props.widget.options = { values: ['downloaded.safetensors'] };
  finish('downloaded.safetensors');
  await operation;
  expect(native.props.widget.value).toBe('downloaded.safetensors');
  expect(native.props.modelValue).toBe('downloaded.safetensors');
  native.graph._nodes.splice(0);
  registry.reconcile();
  expect(() => session.select('downloaded.safetensors')).toThrow('unavailable');
  registry.clear();
  mount?.remove();
});
