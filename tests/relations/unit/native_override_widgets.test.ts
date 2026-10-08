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
/** Prove override controls delegate rendering and value edits to the installed Comfy widget family. */
import { afterEach, expect, jest, test } from '@jest/globals';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import type { OverrideGroup } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import { emptyRelationState } from '../../../frontend/comfyui/ui/relations/domain/RelationContracts.js';
import { readNativeWidgetMount } from '../../../frontend/comfyui/ui/surface/NativeWidgetMountBoundary.js';
import { GlobalOverrideToolbar } from '../../../frontend/comfyui/ui/relations/presentation/GlobalOverrideToolbar.js';
import { WorkflowRelations } from '../../../frontend/comfyui/ui/relations/application/WorkflowRelations.js';
import { NativeOverrideToolbarHost } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeOverrideToolbarHost.js';
import { ContextMenu } from '../../../frontend/comfyui/ui/menus/ContextMenu.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';

const numberComponent = { __name: 'WidgetInputNumber' };
const selectComponent = { __name: 'WidgetSelect' };
const renders = jest.fn<(vnode: unknown, target: HTMLElement) => void>();
const h = jest.fn((type: unknown, props: UnknownRecord) => ({ type, props }));
jest.unstable_mockModule(
  '../../../frontend/comfyui/ui/surface/ComfyNativeWidgetRuntime.js',
  () => ({
    loadComfyNativeWidgetRuntime: async () => ({
      render: renders,
      h,
      components: new Map([
        ['WidgetInputNumber', numberComponent],
        ['WidgetSelect', selectComponent],
      ]),
    }),
  }),
);
const { NativeRelationWidgetRenderer } = await import(
  '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationWidgetRenderer.js'
);

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sc-relation-styles')?.remove();
  renders.mockClear();
  h.mockClear();
});

/** Construct a shared seed surface with two real domain participants. */
function group(): OverrideGroup {
  const domain = { kind: 'number' as const, minimum: 0, maximum: 100, step: 1, integer: true };
  return {
    key: 'seed',
    label: 'Seed',
    domain,
    enabled: true,
    value: 42,
    seedMode: 'fixed',
    members: ['a', 'b'].map((id) => ({
      id,
      nodeId: id,
      cubeId: id,
      label: id,
      name: 'seed',
      semantic: 'seed',
      domain,
      value: 42,
    })),
  };
}

test('uses native component identity, Vue context, and the same seed control boundary as node cards', async () => {
  const app = document.createElement('div');
  app.id = 'vue-app';
  const context = {};
  Reflect.set(app, '__vue_app__', { _context: context });
  const root = document.createElement('div');
  document.body.append(app, root);
  const change = jest.fn();
  const mode = jest.fn();
  const error = jest.fn();
  const renderer = new NativeRelationWidgetRenderer(document, error);
  const mount = renderer.mount(root, group(), change, mode);
  await Promise.resolve();
  await Promise.resolve();
  const boundary = readNativeWidgetMount(root);
  expect(error).not.toHaveBeenCalled();
  expect(boundary?.vnode.type).toBe(numberComponent);
  expect(boundary?.vnode.appContext).toMatchObject({
    ...context,
    config: { idPrefix: expect.stringMatching(/^sc-native-/) },
  });
  const props = boundary?.vnode.props as UnknownRecord;
  const update = props['onUpdate:modelValue'];
  if (typeof update !== 'function') throw new Error('Native value callback missing');
  update(77);
  expect(change).toHaveBeenCalledWith(77);
  boundary?.seed?.writeMode('randomize');
  expect(mode).toHaveBeenCalledWith('randomize');
  mount.update({ ...group(), value: 77, seedMode: 'randomize' });
  expect(readNativeWidgetMount(root)?.seed?.readMode()).toBe('randomize');
  mount.dispose();
  expect(readNativeWidgetMount(root)).toBeNull();
  expect(renders).toHaveBeenLastCalledWith(null, root);
});

test('reserves workflow space below Run and creates every field through the widget renderer', async () => {
  const top = document.createElement('div');
  top.id = 'comfyui-body-top';
  const canvas = document.createElement('div');
  canvas.id = 'graph-canvas-container';
  const panel = document.createElement('div');
  panel.className = 'p-splitterpanel';
  const runRow = document.createElement('div');
  const actionbar = document.createElement('div');
  actionbar.className = 'actionbar-container';
  const graph = document.createElement('div');
  graph.className = 'graph-canvas-panel';
  runRow.append(actionbar);
  canvas.append(panel);
  document.body.append(top, canvas);
  const shared = group();
  let contracts = shared.members;
  let state = emptyRelationState();
  const relations = new WorkflowRelations({
    inventory: () => ({
      nodes: [],
      fields: contracts.map((contract) => ({
        contract,
        read: () => 42,
        write: () => undefined,
      })),
    }),
    load: () => state,
    save: (next) => {
      state = next;
    },
    transaction: (operation) => operation(),
    refresh: () => undefined,
  });
  relations.reconcile();
  const mount = jest.fn((root: HTMLElement) => {
    const label = document.createElement('span');
    label.dataset.testid = 'widget-layout-field-label';
    label.textContent = '<b>Seed</b>';
    label.getBoundingClientRect = () => ({
      x: 0,
      y: 0,
      width: 38,
      height: 33,
      top: 0,
      bottom: 33,
      left: 0,
      right: 38,
      toJSON: () => ({}),
    });
    root.append(label);
    return { update: jest.fn(), dispose: jest.fn() };
  });
  const toolbar = new GlobalOverrideToolbar(
    document,
    relations,
    jest.fn(),
    { mount, mountChoice: mount },
    new NativeOverrideToolbarHost(document),
  );
  toolbar.reconcile();
  expect(mount).not.toHaveBeenCalled();
  expect(toolbar.element.isConnected).toBe(false);
  panel.append(runRow, graph);
  toolbar.reconcile();
  expect(top.contains(toolbar.element)).toBe(false);
  expect(toolbar.element.previousElementSibling).toBe(runRow);
  expect(toolbar.element.nextElementSibling).toBe(graph);
  expect(actionbar.parentElement).toBe(runRow);
  expect(toolbar.element.querySelector('.sc-overrides-title')).toBeNull();
  expect(
    toolbar.element
      .querySelector('.sc-overrides-fields')
      ?.getAttribute('data-sc-scrollbar-horizontal-outset'),
  ).toBe('3');
  expect(toolbar.element.firstElementChild?.getAttribute('aria-label')).toBe(
    'Move global overrides',
  );
  expect(toolbar.element.querySelector('.icon-\\[lucide--plus\\]')).toBeNull();
  expect(mount).toHaveBeenCalledTimes(1);
  ensureRelationStyles(document);
  const grip = toolbar.element.querySelector<HTMLButtonElement>('.sc-overrides-grip')!;
  expect(grip.firstElementChild?.className).toBe('drag-handle h-max w-3 cursor-grab');
  expect(getComputedStyle(grip).width).toBe('12px');
  expect(getComputedStyle(grip).padding).toBe('0px');
  const menuButton = toolbar.element.querySelector<HTMLButtonElement>('.sc-overrides-menu')!;
  const nativeControl = toolbar.element.querySelector('.sc-override-control');
  expect(toolbar.element.dataset.presentation).toBe('comfy');
  expect(getComputedStyle(menuButton).flexDirection).toBe('row');
  expect(getComputedStyle(menuButton).width).toBe('40px');
  expect(getComputedStyle(menuButton.firstElementChild!).width).toBe('16px');
  expect(getComputedStyle(menuButton.lastElementChild!).width).toBe('8px');
  toolbar.setPresentation('substitute');
  expect(getComputedStyle(menuButton).flexDirection).toBe('column');
  expect(getComputedStyle(menuButton).width).toBe('24px');
  expect(getComputedStyle(menuButton.firstElementChild!).width).toBe('14px');
  toolbar.setPresentation('comfy');
  expect(getComputedStyle(menuButton).flexDirection).toBe('row');
  expect(toolbar.element.querySelector('.sc-overrides-menu')).toBe(menuButton);
  expect(toolbar.element.querySelector('.sc-override-control')).toBe(nativeControl);
  expect(mount).toHaveBeenCalledTimes(1);
  await Promise.resolve();
  expect(toolbar.element.querySelector('.sc-override-control')?.getAttribute('style')).toContain(
    '--sc-override-label-width: 38px',
  );
  expect(toolbar.element.querySelector('b')).toBeNull();
  expect(toolbar.element.querySelector('input')).toBeNull();
  const open = jest.spyOn(ContextMenu.prototype, 'open').mockImplementation(() => undefined);
  menuButton.click();
  expect(open.mock.calls[0]?.[0].entries.map((entry) => entry.label)).toEqual(['Seed']);
  expect(open.mock.calls[0]?.[0].entries[0]?.checked).toBe(true);
  open.mockRestore();
  runRow.remove();
  toolbar.reconcile();
  expect(toolbar.element.isConnected).toBe(false);
  panel.insertBefore(runRow, graph);
  toolbar.reconcile();
  expect(toolbar.element.nextElementSibling).toBe(graph);
  expect(mount).toHaveBeenCalledTimes(1);
  contracts = [
    ...shared.members,
    ...['sampler_name', 'scheduler'].map((semantic) => ({
      ...shared.members[0]!,
      id: semantic,
      name: semantic,
      semantic,
      domain: { kind: 'choice' as const, values: ['euler', 'heun'] },
      value: 'euler',
    })),
  ];
  relations.reconcile();
  toolbar.reconcile();
  relations.setOverrideEnabled('sampler_name', false);
  toolbar.reconcile();
  relations.setOverrideEnabled('sampler_name', true);
  toolbar.reconcile();
  expect(
    [...toolbar.element.querySelectorAll<HTMLElement>('.sc-override-control')].map(
      (root) => root.dataset.overrideKey,
    ),
  ).toEqual(['sampler_name', 'scheduler', 'seed']);
  toolbar.dispose();
});

test('renders node and field link choices with the installed select and commits stable relation identities', async () => {
  const app = document.createElement('div');
  app.id = 'vue-app';
  Reflect.set(app, '__vue_app__', { _context: {} });
  const root = document.createElement('div');
  document.body.append(app, root);
  const change = jest.fn();
  const renderer = new NativeRelationWidgetRenderer(document, jest.fn());
  const mount = renderer.mountChoice(root, 'Link sampler', change);
  mount.update('link:cube-a', [
    { value: 'euler', label: 'euler', blockers: [] },
    { value: 'link:cube-a', label: '🔗 Cube A', blockers: [] },
  ]);
  await Promise.resolve();
  await Promise.resolve();
  const vnode = readNativeWidgetMount(root)?.vnode;
  expect(vnode?.type).toBe(selectComponent);
  expect(root.dataset.scChoiceInline).toBe('');
  const props = vnode?.props as UnknownRecord;
  const update = props['onUpdate:modelValue'];
  if (typeof update !== 'function') throw new Error('Native select callback missing');
  update('link:cube-a');
  expect(change).toHaveBeenCalledWith('link:cube-a');
  expect(props.modelValue).toBe('link:cube-a');
  mount.dispose();
});
