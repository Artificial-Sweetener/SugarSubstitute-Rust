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

import { ComfyVueNodeCardRenderer } from '../../../frontend/comfyui/ui/surface/ComfyVueNodeCardRenderer.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { refreshNativeNodeCards } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRefresh.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

describe('ComfyVueNodeCardRenderer', () => {
  test('refreshes published native choices on surviving card mounts and releases the subscription on unmount', () => {
    const target = document.createElement('div');
    const root = document.createElement('div');
    root.className = 'lg-node';
    document.body.append(target);
    let choices: string[] = [];
    const h = jest.fn((_component: unknown, props: unknown) => ({ props }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        h,
        render: (vnode) => {
          if (vnode !== null) target.append(root);
        },
        extractVueNodeData: () => ({
          id: 'empty-native-field',
          widgets: [{ name: 'ckpt_name', type: 'combo', options: { values: [...choices] } }],
        }),
      },
    });
    const mount = renderer.mount(
      target,
      { id: 'empty-native-field', widgets: [{ name: 'ckpt_name', type: 'combo', value: '' }] },
      { preserveNativeBody: true },
    );
    const origin = target.querySelector('.lg-node');
    choices = ['verified-fixture.safetensors'];
    refreshNativeNodeCards(document);
    expect(h).toHaveBeenCalledTimes(2);
    expect(h.mock.lastCall?.[1]).toEqual(
      expect.objectContaining({
        nodeData: expect.objectContaining({
          widgets: [expect.objectContaining({ options: { values: choices } })],
        }),
      }),
    );
    expect(target.querySelector('.lg-node')).toBe(origin);
    mount.unmount();
    refreshNativeNodeCards(document);
    expect(h).toHaveBeenCalledTimes(2);
    target.remove();
  });
  test('blocks native wheel forwarding in masonry while preserving custom wheel handlers and scrolling', () => {
    const surface = document.createElement('section');
    surface.className = 'sugarcubes-workflow-masonry';
    const target = document.createElement('div');
    surface.append(target);
    const root = document.createElement('div');
    root.className = 'lg-node';
    const wrapper = document.createElement('div');
    wrapper.dataset.testid = 'node-inner-wrapper';
    const header = document.createElement('div');
    const customWidget = document.createElement('canvas');
    wrapper.append(header, customWidget);
    const footer = document.createElement('div');
    root.append(wrapper, footer);
    const forwardedWheel = jest.fn();
    const customWheel = jest.fn();
    root.addEventListener('wheel', forwardedWheel);
    customWidget.addEventListener('wheel', customWheel);
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: (vnode) => {
          if (vnode !== null) target.append(root);
        },
        h: () => ({}),
        extractVueNodeData: () => ({ id: 'scroll-proof' }),
      },
    });
    const mount = renderer.mount(target, { id: 'scroll-proof' }, { preserveNativeBody: true });
    for (const element of [root, wrapper, header, footer, customWidget]) {
      for (const modifiers of [{}, { ctrlKey: true }, { metaKey: true }]) {
        const wheel = new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          deltaY: 120,
          ...modifiers,
        });
        element.dispatchEvent(wheel);
        expect(wheel.defaultPrevented).toBe(false);
      }
    }
    expect(customWheel).toHaveBeenCalledTimes(3);
    expect(forwardedWheel).not.toHaveBeenCalled();
    surface.className = '';
    header.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    expect(forwardedWheel).toHaveBeenCalledTimes(1);
    surface.className = 'sugarcubes-workflow-masonry';
    mount.refresh();
    header.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    expect(forwardedWheel).toHaveBeenCalledTimes(1);
    mount.unmount();
    header.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    expect(forwardedWheel).toHaveBeenCalledTimes(2);
  });
  test.each([false, true])(
    'renders a promoted Cube control with its real owner identity and callback (subgraph=%s)',
    (subgraph) => {
      const target = document.createElement('div');
      const owner: ComfyNode = { id: 'cube', widgets: [{ name: 'prompt' }] };
      if (subgraph)
        Object.assign(owner, {
          type: 'boundary-uuid',
          subgraph: {},
          isSubgraphNode: () => true,
          inputs: [{ name: 'prompt', widget: { name: 'prompt' } }],
          resolveSubgraphInputLinks: () => [
            {
              inputNode: {
                id: 'loader',
                type: 'SimpleSyrup.LoadUltralyticsModel',
                inputs: [{ name: 'model_name', widget: { name: 'model_name' } }],
              },
              link: { target_slot: 0 },
            },
          ],
        });
      const internal: ComfyNode = {
        id: 'internal',
        widgets: [{ name: 'text' }],
        inputs: [{ name: 'text', widget: { name: 'text' }, link: 7 }],
      };
      const callback = jest.fn();
      const h = jest.fn();
      const renderer = new ComfyVueNodeCardRenderer({
        component: {},
        appContext: {},
        runtime: {
          render() {},
          h: (_component, props) => {
            h(props);
            return {};
          },
          extractVueNodeData: (node) => ({
            id: node.id,
            widgets: node === owner ? [{ name: 'prompt', widgetId: 'cube:prompt', callback }] : [],
          }),
        },
      });
      const mount = renderer.mount(target, internal, {
        widgetSources: [{ node: owner, name: 'prompt', targetName: 'text' }],
      });
      expect(h).toHaveBeenCalledWith({
        key: false,
        nodeData: expect.objectContaining({
          widgets: [
            {
              name: 'text',
              widgetId: 'cube:prompt',
              callback,
              ...(subgraph
                ? {
                    sugarcubesNativeWidgetOrigins: [
                      { classType: 'SimpleSyrup.LoadUltralyticsModel', inputName: 'model_name' },
                    ],
                  }
                : {}),
            },
          ],
        }),
      });
      mount.unmount();
    },
  );
  test('isolates masonry measurements and preserves widgetless native custom content', async () => {
    const target = document.createElement('div');
    const root = document.createElement('div');
    root.className = 'lg-node';
    root.dataset.nodeId = 'preview';
    const body = document.createElement('div');
    body.dataset.testid = 'node-body-preview';
    const preview = document.createElement('canvas');
    body.append(preview);
    const grid = document.createElement('div');
    grid.dataset.widgetsGridNodeId = 'preview';
    root.append(body, grid);
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: () => target.append(root),
        h: () => ({}),
        extractVueNodeData: () => ({ id: 'preview', widgets: [] }),
      },
    });
    const mount = renderer.mount(
      target,
      { id: 'preview', type: 'PreviewImage', size: [360, 280] },
      { presentationGeometry: true, preserveNativeBody: true },
    );
    expect(root.hasAttribute('data-node-id')).toBe(false);
    expect(grid.hasAttribute('data-widgets-grid-node-id')).toBe(false);
    expect(root.dataset.presentationNodeId).toBe('preview');
    expect(body.hidden).toBe(false);
    expect(preview.hidden).toBe(false);
    const laterGrid = document.createElement('div');
    laterGrid.dataset.widgetsGridNodeId = 'preview';
    root.append(laterGrid);
    await Promise.resolve();
    expect(laterGrid.hasAttribute('data-widgets-grid-node-id')).toBe(false);
    mount.unmount();
  });
  test('retains extracted widget identities and host callbacks in projected cards', () => {
    const rawWidget = { name: 'steps', type: 'number', value: 20 };
    const callback = jest.fn();
    const safeWidget = { name: 'steps', type: 'number', widgetId: 'graph:1:steps', callback };
    const h = jest.fn(() => ({ type: 'node' }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: jest.fn(),
        h,
        extractVueNodeData: () => ({ id: '1', widgets: [safeWidget] }),
      },
    });
    renderer.mount(document.createElement('div'), { id: 1, widgets: [rawWidget] });
    expect(h).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        nodeData: expect.objectContaining({ widgets: [safeWidget] }),
      }),
    );
  });
  test('uses Comfy extraction and component rendering without cloning the graph node', () => {
    const widgets = [{ name: 'steps' }];
    const node: ComfyNode = { id: 'inside-1', type: 'KSampler', widgets };
    const inputs = [{ name: 'model' }];
    const outputs = [{ name: 'LATENT' }];
    const nodeData = { id: 'inside-1', inputs, outputs, widgets };
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
      key: false,
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

  test('projects only widgets whose values are not supplied by graph links', () => {
    const consumed = { name: 'text', value: 'stale prompt' };
    const editable = { name: 'strength', value: 0.7 };
    const widgets = [consumed, editable];
    const node: ComfyNode = {
      id: 'inside-linked',
      type: 'PromptStyler',
      widgets,
      inputs: [{ name: 'text', link: 71, widget: { name: 'text' } }],
    };
    const h = jest.fn(() => ({ type: 'node' }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        render: jest.fn(),
        h,
        extractVueNodeData: () => ({ id: 'inside-linked', widgets }),
      },
    });

    renderer.mount(document.createElement('div'), node);

    expect(h).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        nodeData: expect.objectContaining({ widgets: [editable] }),
      }),
    );
    expect(node.widgets).toBe(widgets);
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
    expect(componentRoots.content.hidden).toBe(false);
    expect(componentRoots.preview.hidden).toBe(false);
    expect(componentRoots.imagePreview.hidden).toBe(false);
    expect(componentRoots.badges.hidden).toBe(true);
    expect(componentRoots.footer.hidden).toBe(true);
    expect(componentRoots.slots.style.getPropertyValue('display')).toBe('none');
    expect(componentRoots.slots.style.getPropertyPriority('display')).toBe('important');
    expect(componentRoots.widgets.hidden).toBe(false);
    expect(nativeRoot.dataset.cubeFacePresentation).toBe('true');
    expect(nativeRoot.style.getPropertyValue('transform')).toBe('translate(40px, 20px)');
    expect(nativeRoot.style.getPropertyValue('--min-node-width')).toBe('');
  });

  test('retains the native advanced footer while expanded and shares disclosure with the context menu', () => {
    const node: ComfyNode = {
      id: 'advanced-node',
      type: 'KSampler',
      showAdvanced: false,
      widgets: [{ name: 'tile_size', type: 'number', value: 512, options: { advanced: true } }],
    };
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const footer = document.createElement('div');
    const advancedInputs = document.createElement('button');
    advancedInputs.dataset.testid = 'advanced-inputs-button';
    footer.append(advancedInputs);
    nativeRoot.append(footer);
    const target = document.createElement('div') as HTMLDivElement & { _vnode?: unknown };
    let projected: unknown;
    const nativeKeys: unknown[] = [];
    const extractVueNodeData = jest.fn(() => ({
      id: 'advanced-node',
      showAdvanced: node.showAdvanced,
    }));
    const renderer = new ComfyVueNodeCardRenderer({
      component: { __name: 'LGraphNode' },
      appContext: {},
      runtime: {
        h: (_component, props) => {
          nativeKeys.push(props.key);
          projected = props.nodeData;
          return { type: 'node' };
        },
        extractVueNodeData,
        render: (vnode) => {
          if (vnode === null) return;
          target.append(nativeRoot);
          target._vnode = {
            component: { subTree: { children: [componentVNode('NodeFooter', footer)] } },
          };
        },
      },
    });
    const mount = renderer.mount(target, node);
    if (!isRecord(projected) || !isRecord(projected.sugarcubesAdvancedInputs))
      throw new Error('Missing advanced capability');
    const control = projected.sugarcubesAdvancedInputs;
    if (typeof control.setShown !== 'function') throw new Error('Missing setter');
    expect(footer.hidden).toBe(false);
    const nativeToggle = jest.fn();
    advancedInputs.addEventListener('click', nativeToggle);
    advancedInputs.click();
    expect(node.showAdvanced).toBe(true);
    expect(nativeToggle).not.toHaveBeenCalled();
    expect(footer.hidden).toBe(false);
    advancedInputs.click();
    expect(node.showAdvanced).toBe(false);
    expect(nativeKeys).toEqual([false, true, false]);
    node.flags = { collapsed: true };
    mount.refresh();
    control.setShown(true);
    expect(node.showAdvanced).toBe(true);
    expect(node.flags.collapsed).toBe(true);
    expect(footer.hidden).toBe(true);
    node.flags.collapsed = false;
    mount.refresh();
    expect(footer.hidden).toBe(false);
    expect(extractVueNodeData).toHaveBeenCalledTimes(6);
    mount.unmount();
    control.setShown(false);
    expect(extractVueNodeData).toHaveBeenCalledTimes(6);
    advancedInputs.click();
    expect(nativeToggle).toHaveBeenCalledTimes(1);
  });

  test('collapses a badge-only card to its native title row', () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const innerWrapper = document.createElement('div');
    innerWrapper.dataset.testid = 'node-inner-wrapper';
    const header = document.createElement('div');
    header.dataset.testid = 'node-header-widgetless';
    const collapse = document.createElement('button');
    collapse.dataset.testid = 'node-collapse-button';
    header.append(collapse);
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

    const node: ComfyNode = {
      id: 'widgetless',
      type: 'MahiroCFG',
      widgets: [],
      flags: { collapsed: true },
      collapse: jest.fn(),
    };
    renderer.mount(target, node);
    expect(collapse.hidden).toBe(true);
    expect(collapse.style.display).toBe('none');
    expect(collapse.disabled).toBe(true);
    collapse.click();
    expect(node.collapse).not.toHaveBeenCalled();
    expect(node.flags?.collapsed).toBe(true);

    expect(badges.hidden).toBe(true);
    expect(body.hidden).toBe(true);
    expect(body.style.getPropertyValue('display')).toBe('none');
    expect(nativeRoot.dataset.cubeFaceBody).toBe('header-only');
  });

  test('routes the native collapse control to the exact projected node and retains its state', () => {
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

    const flags = { collapsed: false, pinned: true };
    const node: ComfyNode = {
      id: 'inside-collapse-control',
      type: 'KSampler',
      widgets: [{ name: 'steps' }],
      flags,
      collapse: jest.fn(() => {
        flags.collapsed = !flags.collapsed;
      }),
    };
    const mount = renderer.mount(target, node);
    const hostAction = jest.fn();
    collapseButton.addEventListener('click', hostAction);
    expect(collapseButton.hidden).toBe(false);
    expect(collapseButton.disabled).toBe(false);
    collapseButton.click();
    expect(node.collapse).toHaveBeenCalledTimes(1);
    expect(hostAction).not.toHaveBeenCalled();
    expect(node.flags).toBe(flags);
    expect(flags).toEqual({ collapsed: true, pinned: true });
    mount.unmount();
    collapseButton.click();
    expect(node.collapse).toHaveBeenCalledTimes(1);
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

  test('preserves native graph collapse in Nodes 2 projected cards without mutating extraction', () => {
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
          flags: { collapsed: true, pinned: true },
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

  test('expands a semantic prompt textarea after workflow hydration without an input event', () => {
    const nativeRoot = document.createElement('div');
    nativeRoot.className = 'lg-node';
    const textarea = document.createElement('textarea');
    let contentHeight = 1;
    Object.defineProperty(textarea, 'scrollHeight', {
      configurable: true,
      get: () => contentHeight,
    });
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

    expect(textarea.style.height).toBe('1px');
    contentHeight = 164;
    textarea.value = 'hydrated multiline workflow prompt';
    expect(textarea.style.height).toBe('164px');
    expect(textarea.style.overflowY).toBe('hidden');
    contentHeight = 220;
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
