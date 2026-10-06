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
/** Safeguard native widget ownership, connection protection and nested-card isolation. */
import { expect, test } from '@jest/globals';
import { readNativeEditorCard } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/NativeEditorCardAdapter.js';

test('reads advanced state from its owner even when the footer has been removed', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card sugarcubes-cube-face__native-card-mount';
  const root = document.createElement('div');
  root.className = 'lg-node';
  const header = document.createElement('div');
  header.className = 'lg-node-header';
  const headerRow = document.createElement('div');
  const titleOwner = document.createElement('div'),
    title = document.createElement('div');
  title.dataset.testid = 'node-title';
  titleOwner.append(title);
  headerRow.append(titleOwner);
  header.append(headerRow);
  root.append(header);
  mount.append(root);
  document.body.append(mount);
  let shown = true;
  Object.assign(mount, {
    _vnode: {
      type: { __name: 'LGraphNode' },
      component: {
        props: {
          nodeData: {
            type: 'VAEEncodeTiled',
            flags: { collapsed: true },
            widgets: [],
            sugarcubesAdvancedInputs: {
              shown: () => shown,
              setShown: (value: boolean) => {
                shown = value;
              },
            },
          },
        },
        subTree: { el: root },
      },
    },
  });
  const card = readNativeEditorCard(root);
  expect(card?.header).toBe(header);
  expect(card?.advanced?.shown()).toBe(true);
  card?.advanced?.setShown(false);
  expect(shown).toBe(false);
  mount.remove();
});

test('adapts owned custom JSON widgets and spec-free advanced controls through their native callbacks', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card sugarcubes-cube-face__native-card-mount';
  const root = document.createElement('div');
  root.className = 'lg-node';
  mount.append(root);
  document.body.append(mount);
  let customApplied: unknown;
  const custom = {
    name: 'layers',
    type: 'layer_stack',
    value: { items: [1, 2, 3], mask: null },
    callback: (value: unknown) => {
      customApplied = value;
    },
  };
  const hidden = {
    name: 'tile_size',
    type: 'number',
    value: 512,
    options: { advanced: true, precision: 0 },
    callback: () => undefined,
  };
  const transient = { ...custom, name: 'preview', serialize: false };
  Object.assign(mount, {
    _vnode: {
      type: { __name: 'LGraphNode' },
      component: {
        props: {
          nodeData: {
            type: 'Layers',
            widgets: [],
            sugarcubesEditorWidgets: [custom, hidden, transient],
          },
        },
        subTree: { el: root },
      },
    },
  });
  const fields = readNativeEditorCard(root)?.fields ?? [];
  expect(fields.map((field) => field.name)).toEqual(['layers', 'tile_size']);
  expect(fields[0]?.kind).toBe('json');
  expect(fields[0]?.write({ items: [4], mask: null })).toBe(true);
  expect(customApplied).toEqual({ items: [4], mask: null });
  expect(fields[0]?.write(['12', 0])).toBe(false);
  expect(fields[1]?.read()).toBe(512);
  expect(fields[1]?.write(768)).toBe(true);
  expect(hidden.value).toBe(768);
  mount.remove();
});
test('adapts only known native fields of the requested node and writes through its Vue emitter', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card sugarcubes-cube-face__native-card-mount';
  const root = document.createElement('div');
  root.className = 'lg-node';
  mount.append(root);
  document.body.append(mount);
  const row = document.createElement('div');
  const control = document.createElement('div');
  const label = document.createElement('label');
  label.textContent = 'Width';
  row.append(label);
  row.append(control);
  const widgets = document.createElement('div');
  widgets.append(row);
  root.append(widgets);
  let value = 1024;
  const props = {
    widget: {
      name: 'width',
      type: 'number',
      options: { min: 1, max: 4096, precision: 0, disabled: false },
    },
    modelValue: value,
  };
  const component = {
    type: { __name: 'WidgetInputNumber' },
    component: {
      props,
      emit: (_event: string, next: number) => {
        value = next;
        props.modelValue = next;
      },
      subTree: { el: control },
    },
  };
  Object.assign(mount, {
    _vnode: {
      type: { __name: 'LGraphNode' },
      component: {
        props: {
          nodeData: {
            type: 'EmptyLatentImage',
            title: 'Latent',
            widgets: [{ name: 'width', type: 'number', value: 1024 }],
          },
        },
        subTree: {
          el: root,
          children: [
            {
              type: { __name: 'NodeWidgets' },
              component: { subTree: { el: widgets, children: [component] } },
            },
          ],
        },
      },
    },
  });
  const card = readNativeEditorCard(root);
  expect(card?.nodeType).toBe('EmptyLatentImage');
  expect(card?.fields).toHaveLength(1);
  expect(card?.fields[0]?.element).toBe(row);
  expect(card?.fields[0]?.element.contains(label)).toBe(true);
  expect(card?.fields[0]?.write(576)).toBe(true);
  expect(value).toBe(576);
  expect(card?.fields[0]?.write(8192)).toBe(true);
  expect(value).toBe(4096);
  props.widget.options = { ...props.widget.options, min: 64, max: 2048 };
  expect(card?.fields[0]?.write(16)).toBe(true);
  expect(value).toBe(64);
  expect(card?.fields[0]?.write(2048.5)).toBe(false);
  expect(value).toBe(64);
  props.widget.options = { ...props.widget.options, disabled: true };
  expect(card?.fields[0]?.write(768)).toBe(false);
  expect(value).toBe(64);
  const nested = document.createElement('div');
  nested.className = 'lg-node';
  nested.append(control);
  root.append(nested);
  expect(readNativeEditorCard(root)?.fields).toEqual([]);
  mount.remove();
});

test('captures declared advanced inputs without rendering them and protects linked inputs', () => {
  const mount = document.createElement('div');
  mount.className = 'sugarcubes-native-node-card sugarcubes-cube-face__native-card-mount';
  const root = document.createElement('div');
  root.className = 'lg-node';
  mount.append(root);
  document.body.append(mount);
  let applied = 0;
  const advanced = {
    name: 'tile_size',
    type: 'number',
    value: 512,
    options: { min: 1, max: 4096 },
    spec: { type: 'INT' },
    callback: (value: number) => {
      applied = value;
    },
  };
  const linked = { ...advanced, name: 'overlap', slotMetadata: { linked: true } };
  Object.assign(mount, {
    _vnode: {
      type: { __name: 'LGraphNode' },
      component: {
        props: {
          nodeData: {
            type: 'VAEEncodeTiled',
            title: 'VAE Encode',
            widgets: [],
            sugarcubesEditorWidgets: [advanced, linked],
          },
        },
        subTree: { el: root },
      },
    },
  });
  const fields = readNativeEditorCard(root)?.fields ?? [];
  expect(fields.map((field) => field.name)).toEqual(['tile_size', 'overlap']);
  expect(fields[0]?.read()).toBe(512);
  expect(fields[0]?.write(1024)).toBe(true);
  expect(applied).toBe(1024);
  expect(fields[0]?.write(1024.5)).toBe(false);
  expect(fields[0]?.write(8192)).toBe(true);
  expect(applied).toBe(4096);
  advanced.options = { min: 64, max: 2048 };
  expect(fields[0]?.write(16)).toBe(true);
  expect(applied).toBe(64);
  expect(fields[1]?.disabled()).toBe(true);
  expect(fields[1]?.write(2048)).toBe(false);
  mount.remove();
});
