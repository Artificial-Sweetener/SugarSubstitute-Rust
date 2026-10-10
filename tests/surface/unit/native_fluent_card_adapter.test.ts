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
/** Prove capability boundaries and reversible native field grouping. */
import { afterEach, expect, test } from '@jest/globals';
import { NativeFluentCardAdapter } from '../../../frontend/comfyui/ui/surface/appearance/NativeFluentCardAdapter.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';
import {
  registerNativeWidgetMount,
  releaseNativeWidgetMount,
} from '../../../frontend/comfyui/ui/surface/NativeWidgetMountBoundary.js';

afterEach(() => document.body.replaceChildren());

test('retains live native widget presentation through disconnection until its mount is released', () => {
  const adapter = new NativeFluentCardAdapter();
  const root = document.createElement('div');
  root.className = 'sugarcubes-native-widget';
  const trigger = document.createElement('button');
  root.append(trigger);
  document.body.append(root);
  registerNativeWidgetMount(root, { vnode: {}, seed: null });
  adapter.annotateWidget(root);
  root.remove();
  adapter.endBatch();
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(true);
  document.body.append(root);
  expect(root.querySelector('button')).toBe(trigger);
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(true);
  releaseNativeWidgetMount(root);
  root.remove();
  adapter.endBatch();
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(false);
  adapter.clear();
});

test('retains a relation control temporarily detached while another native field is reconciled', () => {
  const adapter = new NativeFluentCardAdapter();
  const first = document.createElement('div');
  const second = document.createElement('div');
  first.className = second.className = 'sugarcubes-native-widget';
  const trigger = document.createElement('button');
  first.append(trigger);
  document.body.append(first, second);
  adapter.annotateWidget(first);
  first.remove();
  adapter.annotateWidget(second);
  document.body.append(first);
  adapter.endBatch();
  expect(first.hasAttribute('data-sc-fluent-node')).toBe(true);
  expect(first.querySelector('button')).toBe(trigger);
  second.remove();
  adapter.endBatch();
  expect(second.hasAttribute('data-sc-fluent-node')).toBe(false);
  adapter.clear();
});

test('releases detached native annotations on an empty retirement batch', () => {
  const root = document.createElement('div');
  root.className = 'lg-node';
  document.body.append(root);
  const adapter = new NativeFluentCardAdapter();
  adapter.annotate(root);
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(true);
  root.remove();
  adapter.endBatch();
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(false);
});

test.each([false, true])(
  'owns only this graph card’s native port section in masonry=%s',
  (masonry) => {
    const surface = document.createElement('div');
    if (masonry) surface.className = 'sugarcubes-cube-face__node-card';
    const mount = document.createElement('div');
    mount.className = 'sugarcubes-native-node-card';
    const root = document.createElement('div');
    root.className = 'lg-node';
    const slots = document.createElement('div');
    const port = document.createElement('div');
    port.className = 'lg-slot';
    slots.append(port);
    const fields = document.createElement('div');
    const field = document.createElement('div');
    fields.append(field);
    const nested = document.createElement('div');
    nested.className = 'lg-node';
    const nestedSlots = document.createElement('div');
    nested.append(nestedSlots);
    root.append(slots, fields, nested);
    mount.append(root);
    surface.append(mount);
    document.body.append(surface);
    Object.assign(mount, {
      _vnode: vnode('LGraphNode', root, [
        vnode('NodeSlots', slots, []),
        vnode('NodeWidgets', fields, []),
        vnode('LGraphNode', nested, [vnode('NodeSlots', nestedSlots, [])]),
      ]),
    });
    const adapter = new NativeFluentCardAdapter();
    adapter.annotate(root);
    expect(slots.hasAttribute('data-sc-fluent-ports')).toBe(!masonry);
    expect(nestedSlots.hasAttribute('data-sc-fluent-ports')).toBe(false);
    expect(port.parentElement).toBe(slots);
    field.hidden = true;
    adapter.annotate(root);
    expect(slots.hasAttribute('data-sc-fluent-ports')).toBe(false);
    field.hidden = false;
    adapter.annotate(root);
    port.remove();
    adapter.annotate(root);
    expect(slots.hasAttribute('data-sc-fluent-ports')).toBe(false);
    slots.append(port);
    adapter.annotate(root);
    adapter.clear();
    expect(slots.hasAttribute('data-sc-fluent-ports')).toBe(false);
    expect(port.parentElement).toBe(slots);
  },
);

test('retains identical annotations and removes only properties whose field ownership changed', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card';
  const root = document.createElement('div');
  root.className = 'lg-node';
  const fields = document.createElement('div');
  const row = document.createElement('div');
  const control = document.createElement('div');
  control.append(document.createElement('input'));
  row.append(control);
  fields.append(row);
  root.append(fields);
  mount.append(root);
  document.body.append(mount);
  const widgets = vnode('NodeWidgets', fields, [
    vnode('WidgetInputText', control, [], { widget: { name: 'text' } }),
  ]);
  Object.assign(mount, { _vnode: vnode('LGraphNode', root, [widgets]) });
  const adapter = new NativeFluentCardAdapter();
  adapter.annotate(root);
  const observer = new MutationObserver(() => undefined);
  observer.observe(root, { attributes: true, subtree: true });
  adapter.annotate(root);
  expect(observer.takeRecords()).toHaveLength(0);
  Object.assign(widgets, { component: { subTree: { el: fields, children: [] } } });
  adapter.annotate(root);
  expect(row.hasAttribute('data-sc-fluent-row')).toBe(false);
  expect(row.style.getPropertyValue('--sc-field-column')).toBe('');
  expect(control.hasAttribute('data-sc-fluent-control')).toBe(false);
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(true);
  observer.disconnect();
  adapter.clear();
});

/** Model only the mounted Vue properties consumed at the host boundary. */
function vnode(
  name: string,
  element: HTMLElement,
  children: UnknownRecord[],
  props: UnknownRecord = {},
): UnknownRecord {
  return { type: { __name: name }, component: { props, subTree: { el: element, children } } };
}

test('groups core fields and releases their annotations when replaced by a custom component', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card';
  const root = document.createElement('div');
  root.className = 'lg-node';
  const fields = document.createElement('div');
  root.append(fields);
  mount.append(root);
  document.body.append(mount);
  const controls: HTMLElement[] = [];
  const nodes = ['steps', 'cfg', 'special'].map((name) => {
    const row = document.createElement('div');
    const control = document.createElement('div');
    const label = document.createElement('span');
    label.dataset.testid = 'widget-layout-field-label';
    label.textContent = name;
    const input = document.createElement('input');
    input.value = '<img src=x>';
    input.style.backgroundColor = 'rgb(12, 34, 56)';
    control.append(label, input);
    row.append(control);
    fields.append(row);
    controls.push(control);
    return vnode(
      name === 'special' ? 'WidgetDOM' : 'WidgetInputNumber',
      control,
      [vnode('WidgetLayoutField', control, [])],
      { widget: { name } },
    );
  });
  Object.assign(mount, {
    _vnode: vnode('LGraphNode', root, [vnode('NodeWidgets', fields, nodes)]),
  });
  const adapter = new NativeFluentCardAdapter();
  const card = adapter.annotate(root);
  expect(card.labelElements.map((label) => label.textContent)).toEqual(['steps', 'cfg']);
  expect(fields.children[0]?.getAttribute('data-sc-fluent-span')).toBe('1');
  expect(fields.children[1]?.getAttribute('style')).toContain('--sc-field-column: 2');
  expect(fields.children[1]?.getAttribute('data-sc-fluent-column')).toBe('2');
  expect(controls[2]?.hasAttribute('data-sc-fluent-control')).toBe(false);
  const hidden = fields.children[1];
  if (!(hidden instanceof HTMLElement)) throw new Error('Second native field missing');
  const nativeInput = controls[0]?.querySelector('input');
  hidden.setAttribute('data-sc-search-field-hidden', '');
  adapter.annotate(root);
  expect(fields.children[0]?.getAttribute('data-sc-fluent-span')).toBe('2');
  expect(controls[0]?.querySelector('input')).toBe(nativeInput);
  hidden.removeAttribute('data-sc-search-field-hidden');
  adapter.annotate(root);
  expect(fields.children[0]?.getAttribute('data-sc-fluent-span')).toBe('1');
  const first = nodes[0];
  if (!first) throw new Error('Fixture component missing');
  first.type = { __name: 'WidgetDOM' };
  adapter.annotate(root);
  expect(controls[0]?.hasAttribute('data-sc-fluent-control')).toBe(false);
  expect(fields.children[0]?.hasAttribute('data-sc-fluent-row')).toBe(false);
  expect(fields.children[1]?.getAttribute('data-sc-fluent-span')).toBe('2');
  adapter.clear();
  expect(root.hasAttribute('data-sc-fluent-node')).toBe(false);
  expect(fields.querySelector('[data-sc-fluent-control]')).toBeNull();
  expect(fields.querySelector('[data-sc-fluent-column]')).toBeNull();
  expect(fields.querySelector('input')?.value).toBe('<img src=x>');
  expect(fields.querySelector('input')?.style.backgroundColor).toBe('rgb(12, 34, 56)');
  expect(fields.querySelector('img')).toBeNull();
});

test.each([[], {}])(
  'reads widgetless and advanced-only collapse capability from native widgets %j',
  (widgets) => {
    const mount = document.createElement('div');
    mount.className = 'sugarcubes-native-node-card';
    const root = document.createElement('div');
    root.className = 'lg-node';
    const footer = document.createElement('div');
    root.append(footer);
    mount.append(root);
    document.body.append(mount);
    const footerVNode = vnode('NodeFooter', footer, [], { showAdvancedInputsButton: false });
    Object.assign(mount, {
      _vnode: vnode('LGraphNode', root, [footerVNode], { nodeData: { widgets } }),
    });
    const adapter = new NativeFluentCardAdapter();
    expect(adapter.annotate(root).canCollapse).toBe(false);
    const component = footerVNode.component;
    if (!component || typeof component !== 'object' || !('props' in component))
      throw new Error('Footer fixture missing');
    Object.assign(component.props ?? {}, { showAdvancedInputsButton: true });
    expect(adapter.annotate(root).canCollapse).toBe(true);
    adapter.clear();
  },
);

test.each(['WidgetSelect', 'WidgetDOM'])(
  'annotates the native default combo frame only within %s boundaries and restores it',
  (componentName) => {
    const mount = document.createElement('div');
    mount.className = 'sugarcubes-native-node-card';
    const root = document.createElement('div');
    root.className = 'lg-node';
    const fields = document.createElement('div');
    const row = document.createElement('div');
    const control = document.createElement('div');
    const frame = document.createElement('div');
    frame.setAttribute('data-capture-wheel', 'true');
    const trigger = document.createElement('button');
    trigger.dataset.testid = 'widget-select-default-trigger';
    trigger.textContent = '<literal selection>';
    frame.append(trigger);
    control.append(frame);
    row.append(control);
    fields.append(row);
    root.append(fields);
    mount.append(root);
    document.body.append(mount);
    Object.assign(mount, {
      _vnode: vnode('LGraphNode', root, [
        vnode('NodeWidgets', fields, [
          vnode(componentName, control, [], { widget: { name: 'sampler_name' } }),
        ]),
      ]),
    });
    const adapter = new NativeFluentCardAdapter();
    adapter.annotate(root);
    expect(frame.hasAttribute('data-sc-fluent-combo-frame')).toBe(componentName === 'WidgetSelect');
    expect(frame.hasAttribute('data-sc-fluent-edit-frame')).toBe(componentName === 'WidgetSelect');
    adapter.clear();
    expect(frame.hasAttribute('data-sc-fluent-combo-frame')).toBe(false);
    expect(frame.hasAttribute('data-sc-fluent-edit-frame')).toBe(false);
    expect(trigger.textContent).toBe('<literal selection>');
    expect(trigger.children).toHaveLength(0);
  },
);

test('does not claim an embedded Cube card advanced toggle from its enclosing native node', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card';
  const root = document.createElement('div');
  root.className = 'lg-node';
  const ownFields = document.createElement('div');
  const embedded = document.createElement('div');
  embedded.className = 'lg-node';
  const fields = document.createElement('div');
  fields.dataset.scFluentFields = '';
  const button = document.createElement('button');
  button.dataset.testid = 'advanced-inputs-button';
  embedded.append(fields, button);
  root.append(ownFields, embedded);
  mount.append(root);
  document.body.append(mount);
  Object.assign(mount, {
    _vnode: vnode('LGraphNode', root, [vnode('NodeWidgets', ownFields, [])]),
  });
  const adapter = new NativeFluentCardAdapter();
  expect(adapter.annotate(root).advanced).toBeNull();
  expect(button.isConnected).toBe(true);
  expect(fields.dataset.scFluentFields).toBe('');
  adapter.clear();
});
