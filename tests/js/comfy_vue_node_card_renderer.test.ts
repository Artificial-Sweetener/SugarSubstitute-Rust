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
/** Characterize native Nodes 2.0 mounting against exact internal nodes. */

import { describe, expect, jest, test } from '@jest/globals';

import { ComfyVueNodeCardRenderer } from '../../frontend/comfyui/ui/surface/ComfyVueNodeCardRenderer.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('ComfyVueNodeCardRenderer', () => {
  test('uses Comfy extraction and component rendering without cloning the graph node', () => {
    const node: ComfyNode = { id: 'inside-1', type: 'KSampler' };
    const inputs = [{ name: 'model' }];
    const outputs = [{ name: 'LATENT' }];
    const nodeData = { id: 'inside-1', inputs, outputs, widgets: [{ name: 'steps' }] };
    const component = { __name: 'LGraphNode' };
    const appContext = { provides: {} };
    const render = jest.fn();
    const h = jest.fn(() => ({ type: component, props: { nodeData } }));
    const extractVueNodeData = jest.fn(() => nodeData);
    const renderer = new ComfyVueNodeCardRenderer({
      component,
      appContext,
      runtime: { render, h, extractVueNodeData },
    });
    const target = document.createElement('div');

    renderer.mount(target, node);

    expect(extractVueNodeData).toHaveBeenCalledWith(node);
    expect(h).toHaveBeenCalledWith(component, {
      nodeData: expect.objectContaining({
        id: 'inside-1',
        inputs: [],
        outputs: [],
        widgets: nodeData.widgets,
        flags: { collapsed: false },
      }),
    });
    expect(render).toHaveBeenCalledWith(
      expect.objectContaining({ appContext, type: component }),
      target,
    );
    expect(nodeData.inputs).toBe(inputs);
    expect(nodeData.outputs).toBe(outputs);
  });

  test('unmounts through Comfy Vue rendering and re-extracts current node state', () => {
    const node: ComfyNode = { id: 'inside-1', type: 'KSampler', mode: 4 };
    const render = jest.fn();
    const extractVueNodeData = jest.fn((current: ComfyNode) => ({
      id: 'inside-1',
      mode: current.mode,
    }));
    const h = jest.fn(() => ({ type: 'node' }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render,
        h,
        extractVueNodeData,
      },
    });
    const first = renderer.mount(document.createElement('div'), node);
    node.mode = 0;
    const second = renderer.mount(document.createElement('div'), node);
    second.refresh();

    first.unmount();
    second.unmount();

    expect(extractVueNodeData).toHaveBeenCalledTimes(3);
    expect(h).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        nodeData: expect.objectContaining({ mode: 0 }),
      }),
    );
    expect(render).toHaveBeenCalledWith(null, expect.any(HTMLElement));
  });

  test('applies Cube-face mode to native component roots without replacing widgets', () => {
    const componentRoots = {
      slots: document.createElement('div'),
      widgets: document.createElement('div'),
      content: document.createElement('div'),
      preview: document.createElement('div'),
      imagePreview: document.createElement('div'),
      badges: document.createElement('div'),
      footer: document.createElement('div'),
    };
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    nativeRoot.style.transform = 'translate(40px, 20px)';
    const target = document.createElement('div') as HTMLDivElement & { _vnode?: unknown };
    const render = jest.fn((vnode: unknown) => {
      if (vnode === null) return;
      target.append(nativeRoot);
      target._vnode = {
        component: {
          subTree: {
            children: [
              componentVNode('NodeSlots', componentRoots.slots),
              componentVNode('NodeWidgets', componentRoots.widgets),
              componentVNode('NodeContent', componentRoots.content),
              componentVNode('LivePreview', componentRoots.preview),
              componentVNode('ImagePreview', componentRoots.imagePreview),
              componentVNode('NodeBadges', componentRoots.badges),
              componentVNode('NodeFooter', componentRoots.footer),
            ],
          },
        },
      };
    });
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render,
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'inside-1' }),
      },
    });

    renderer.mount(target, { id: 'inside-1', type: 'KSampler' });

    expect(componentRoots.slots.hidden).toBe(true);
    expect(componentRoots.content.hidden).toBe(true);
    expect(componentRoots.preview.hidden).toBe(true);
    expect(componentRoots.imagePreview.hidden).toBe(true);
    expect(componentRoots.badges.hidden).toBe(true);
    expect(componentRoots.footer.hidden).toBe(true);
    expect(componentRoots.slots.style.getPropertyValue('display')).toBe('none');
    expect(componentRoots.slots.style.getPropertyPriority('display')).toBe('important');
    expect(componentRoots.widgets.hidden).toBe(false);
    expect(nativeRoot.dataset.cubeFacePresentation).toBe('true');
    expect(nativeRoot.style.getPropertyValue('transform')).toBe('translate(40px, 20px)');
    expect(nativeRoot.style.getPropertyValue('--min-node-width')).toBe('');
  });

  test('preserves Comfy native advanced-input controls and refreshes after interaction', async () => {
    const node: ComfyNode = {
      id: 'advanced-node',
      type: 'KSampler',
      showAdvanced: false,
    };
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const footer = document.createElement('div');
    const advancedInputs = document.createElement('button');
    advancedInputs.dataset.testid = 'advanced-inputs-button';
    advancedInputs.textContent = 'Show advanced inputs';
    const toggleAdvanced = jest.fn();
    let draggingNativeNode = false;
    let suppressNextClick = false;
    nativeRoot.addEventListener('pointerdown', () => {
      draggingNativeNode = true;
    });
    nativeRoot.addEventListener('pointerup', () => {
      draggingNativeNode = false;
    });
    advancedInputs.addEventListener('pointerup', () => {
      suppressNextClick = draggingNativeNode;
    });
    advancedInputs.addEventListener('click', () => {
      if (suppressNextClick) {
        suppressNextClick = false;
        return;
      }
      node.showAdvanced = !node.showAdvanced;
      toggleAdvanced();
    });
    footer.append(advancedInputs);
    nativeRoot.append(footer);
    const target = document.createElement('div') as HTMLDivElement & { _vnode?: unknown };
    const extractVueNodeData = jest.fn(() => ({
      id: 'advanced-node',
      showAdvanced: node.showAdvanced,
    }));
    const h = jest.fn((_component: unknown, props: unknown) => ({ type: 'node', props }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode === null) return;
          target.append(nativeRoot);
          target._vnode = {
            component: {
              subTree: {
                children: [componentVNode('NodeFooter', footer)],
              },
            },
          };
        },
        h,
        extractVueNodeData,
      },
    });

    const mount = renderer.mount(target, node);
    clickNativeControl(advancedInputs);
    await Promise.resolve();

    expect(footer.hidden).toBe(false);
    expect(footer.style.getPropertyValue('display')).toBe('');
    expect(advancedInputs.hidden).toBe(false);
    expect(toggleAdvanced).toHaveBeenCalledTimes(1);
    expect(extractVueNodeData).toHaveBeenCalledTimes(2);
    expect(h).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        nodeData: expect.objectContaining({ showAdvanced: true }),
      }),
    );

    clickNativeControl(advancedInputs);
    await Promise.resolve();
    expect(extractVueNodeData).toHaveBeenCalledTimes(3);
    expect(h).toHaveBeenLastCalledWith(
      expect.anything(),
      expect.objectContaining({
        nodeData: expect.objectContaining({ showAdvanced: false }),
      }),
    );

    mount.unmount();
    clickNativeControl(advancedInputs);
    await Promise.resolve();
    expect(extractVueNodeData).toHaveBeenCalledTimes(3);
  });

  test('collapses a badge-only card to its native title row', () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const innerWrapper = document.createElement('div');
    innerWrapper.dataset.testid = 'node-inner-wrapper';
    const header = document.createElement('div');
    header.dataset.testid = 'node-header-widgetless';
    const body = document.createElement('div');
    body.dataset.testid = 'node-body-widgetless';
    const badges = document.createElement('div');
    badges.textContent = 'BETA';
    body.append(badges);
    innerWrapper.append(header, body);
    nativeRoot.append(innerWrapper);
    const target = document.createElement('div') as HTMLDivElement & { _vnode?: unknown };
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode === null) return;
          target.append(nativeRoot);
          target._vnode = {
            component: {
              subTree: {
                children: [componentVNode('NodeBadges', badges)],
              },
            },
          };
        },
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'widgetless' }),
      },
    });

    renderer.mount(target, { id: 'widgetless', type: 'Mahiro', widgets: [] });

    expect(badges.hidden).toBe(true);
    expect(body.hidden).toBe(true);
    expect(body.style.getPropertyValue('display')).toBe('none');
    expect(nativeRoot.dataset.cubeFaceBody).toBe('header-only');
  });

  test('hides the native Nodes 2 collapse control from a Cube card header', () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const collapseButton = document.createElement('button');
    collapseButton.dataset.testid = 'node-collapse-button';
    nativeRoot.append(collapseButton);
    const target = document.createElement('div');
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode !== null) target.append(nativeRoot);
        },
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'inside-collapse-control' }),
      },
    });

    renderer.mount(target, { id: 'inside-collapse-control', type: 'KSampler' });

    expect(collapseButton.hidden).toBe(true);
    expect(collapseButton.style.getPropertyValue('display')).toBe('none');
    expect(collapseButton.style.getPropertyPriority('display')).toBe('important');
  });

  test('adopts a header accessory into the exact native title row and reconciles replacement', async () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const first = nativeCardHeader('inside-accessory');
    const projected = nativeCardHeader('projected-child');
    nativeRoot.append(first.header, projected.header);
    const target = document.createElement('div');
    const accessory = document.createElement('label');
    accessory.dataset.cubeCardActivation = 'inside-accessory';
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode !== null) target.append(nativeRoot);
        },
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'inside-accessory' }),
      },
    });

    const mount = renderer.mount(
      target,
      { id: 'inside-accessory', type: 'MahiroCFG' },
      { headerAccessory: accessory },
    );

    expect(accessory.parentElement).toBe(first.row);
    expect(first.header.dataset.sugarcubeCardHeaderAccessory).toBe('');
    expect(projected.header.contains(accessory)).toBe(false);

    const replacement = nativeCardHeader('inside-accessory');
    first.header.replaceWith(replacement.header);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(accessory.parentElement).toBe(replacement.row);
    expect(replacement.header.dataset.sugarcubeCardHeaderAccessory).toBe('');

    mount.unmount();
    expect(accessory.isConnected).toBe(false);
    expect(replacement.header.dataset.sugarcubeCardHeaderAccessory).toBeUndefined();
  });

  test('preserves the native node body when Comfy mounts it after the initial render', async () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const target = document.createElement('div') as HTMLDivElement & { _vnode?: unknown };
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode !== null) {
            target.append(nativeRoot);
            target._vnode = { type: { __name: 'LGraphNode' } };
          }
        },
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'inside-1' }),
      },
    });
    renderer.mount(target, { id: 'inside-1', type: 'PreviewImage' });

    const nativeNodeBody = document.createElement('div');
    nativeNodeBody.className = 'lg-node-content';
    const nativeControl = document.createElement('input');
    nativeNodeBody.append(nativeControl);
    nativeRoot.append(nativeNodeBody);
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(nativeNodeBody.hidden).toBe(false);
    expect(nativeNodeBody.style.getPropertyValue('display')).toBe('');
    expect(nativeControl.hidden).toBe(false);
  });

  test('renders collapsed graph nodes expanded without changing their editor state', () => {
    const graphFlags = { collapsed: true, pinned: true };
    const extractedFlags = { collapsed: true, pinned: true };
    const node: ComfyNode = {
      id: 'inside-collapsed',
      type: 'KSampler',
      flags: graphFlags,
    };
    const h = jest.fn(() => ({ type: 'node' }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: jest.fn(),
        h,
        extractVueNodeData: () => ({
          id: 'inside-collapsed',
          flags: extractedFlags,
          inputs: [{ name: 'model' }],
          outputs: [{ name: 'LATENT' }],
        }),
      },
    });

    renderer.mount(document.createElement('div'), node);

    expect(h).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        nodeData: expect.objectContaining({
          flags: { collapsed: false, pinned: true },
          inputs: [],
          outputs: [],
        }),
      }),
    );
    expect(node.flags).toBe(graphFlags);
    expect(node.flags?.collapsed).toBe(true);
    expect(extractedFlags.collapsed).toBe(true);
  });

  test('suppresses the current native subgraph footer without replacing its card', () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const nativeFooter = document.createElement('div');
    nativeFooter.className = 'isolate -z-1 -mt-5 box-border flex w-full';
    const enterSubgraph = document.createElement('button');
    enterSubgraph.textContent = 'Localized subgraph action';
    nativeFooter.append(enterSubgraph);
    const subgraphIcon = document.createElement('i');
    subgraphIcon.className = 'icon-[comfy--workflow]';
    nativeRoot.append(subgraphIcon, nativeFooter);
    const target = document.createElement('div');
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode !== null) target.append(nativeRoot);
        },
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'inside-subgraph' }),
      },
    });

    renderer.mount(target, {
      id: 'inside-subgraph',
      type: 'Subgraph',
      isSubgraphNode: () => true,
    });

    expect(nativeFooter.hidden).toBe(true);
    expect(nativeFooter.style.getPropertyValue('display')).toBe('none');
    expect(nativeRoot.hidden).toBe(false);
    expect(subgraphIcon.hidden).toBe(true);
    expect(subgraphIcon.style.getPropertyValue('display')).toBe('none');
  });

  test('expands a semantic prompt textarea inside a Nodes 2.0 Cube card', () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const textarea = document.createElement('textarea');
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 164 });
    nativeRoot.append(textarea);
    const target = document.createElement('div');
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode !== null) target.append(nativeRoot);
        },
        h: () => ({ type: 'node' }),
        extractVueNodeData: () => ({ id: 'prompt' }),
      },
    });

    renderer.mount(target, {
      id: 'prompt',
      type: 'CLIPTextEncode',
      title: 'Positive prompt',
      widgets: [{ name: 'text', type: 'customtext' }],
    });

    expect(textarea.style.height).toBe('164px');
    expect(textarea.style.overflowY).toBe('hidden');
    Object.defineProperty(textarea, 'scrollHeight', { configurable: true, value: 220 });
    textarea.dispatchEvent(new Event('input'));
    expect(textarea.style.height).toBe('220px');
  });
});

function componentVNode(name: string, element: HTMLElement): unknown {
  return {
    type: { __name: name },
    component: {
      subTree: {
        el: element,
      },
    },
  };
}

/** Dispatch the complete pointer sequence produced by a real control click. */
function clickNativeControl(control: HTMLElement): void {
  control.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  control.dispatchEvent(new Event('pointerup', { bubbles: true }));
  control.click();
}

/** Build the semantic title hierarchy shared by supported Comfy NodeHeader releases. */
function nativeCardHeader(nodeId: string): {
  header: HTMLDivElement;
  row: HTMLDivElement;
} {
  const header = document.createElement('div');
  header.dataset.testid = `node-header-${nodeId}`;
  const row = document.createElement('div');
  const titleOwner = document.createElement('div');
  const title = document.createElement('div');
  title.dataset.testid = 'node-title';
  titleOwner.append(title);
  row.append(titleOwner);
  header.append(row);
  return { header, row };
}
