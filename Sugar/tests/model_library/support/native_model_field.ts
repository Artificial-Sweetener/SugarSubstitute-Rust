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
/** Model the native Vue field boundary for adapter and host lifecycle proofs. */
import { jest } from '@jest/globals';
import type { ModelCatalogSnapshot } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';

/** Mount an observed native Vue boundary with independently recorded host callbacks. */
export function nativeModelFieldFixture(componentName = 'WidgetSelectDefault', paired = false) {
  const mount = document.createElement('div');
  mount.id = 'vue-app';
  const card = document.createElement('div');
  card.className = 'lg-node';
  card.dataset.nodeId = 'checkpoint';
  const frame = document.createElement('div');
  frame.setAttribute('data-capture-wheel', 'true');
  const trigger = document.createElement('button');
  trigger.dataset.testid = 'widget-select-default-trigger';
  frame.append(trigger);
  card.append(frame);
  mount.append(card);
  document.body.append(mount);
  const props: {
    widget: {
      name: string;
      value: string;
      options: { values: string[] };
      callback?: (value: string) => void;
    };
    modelValue: string | null | undefined;
  } = {
    widget: {
      name: 'ckpt_name',
      value: 'Flux\\alpha.safetensors',
      options: { values: ['Flux\\alpha.safetensors', 'None'] },
    },
    modelValue: 'Flux\\alpha.safetensors',
  };
  const stop = jest.fn();
  const emit = jest.fn((_event: string, value: string) => {
    props.modelValue = value;
    props.widget.value = value;
  });
  props.widget.callback = (value) => {
    props.modelValue = value;
  };
  const watch = jest.fn(() => stop);
  const fieldVNode = {
    type: { __name: componentName },
    component: { props, emit, proxy: { $watch: watch }, subTree: { el: frame } },
  };
  const nodeData = { id: 'checkpoint', type: 'CheckpointLoaderSimple', widgets: [props.widget] };
  const children = [fieldVNode];
  if (paired) {
    const sibling = document.createElement('div');
    sibling.setAttribute('data-capture-wheel', 'true');
    const siblingTrigger = document.createElement('button');
    siblingTrigger.dataset.testid = 'widget-select-default-trigger';
    sibling.append(siblingTrigger);
    card.append(sibling);
    const siblingProps = {
      widget: { name: 'vae_name', value: 'None', options: { values: ['None'] } },
      modelValue: 'None',
    };
    nodeData.widgets.push(siblingProps.widget);
    children.push({
      type: { __name: componentName },
      component: { props: siblingProps, emit, proxy: { $watch: watch }, subTree: { el: sibling } },
    });
  }
  Object.assign(mount, {
    _vnode: {
      type: { __name: 'LGraphNode' },
      component: {
        props: { nodeData },
        subTree: { el: card, children },
      },
    },
  });
  const snapshot: ModelCatalogSnapshot = {
    revision: 1,
    items: [],
    scanning: false,
    fieldContracts: [
      { classType: 'CheckpointLoaderSimple', inputName: 'ckpt_name', kind: 'checkpoints' },
      ...(paired
        ? [{ classType: 'CheckpointLoaderSimple', inputName: 'vae_name', kind: 'vae' }]
        : []),
    ],
  };
  const graph = { id: 'root', _nodes: [nodeData] };
  return { card, frame, trigger, props, nodeData, graph, emit, watch, stop, snapshot };
}
