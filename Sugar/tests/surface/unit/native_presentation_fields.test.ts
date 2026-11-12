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
/** Prove native presentation fields retain host identity without entering graph data or duplicating commits. */
import { jest } from '@jest/globals';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { NativeNodeFields } from '../../../frontend/comfyui/ui/surface/NativeNodeFields.js';
import {
  nativePresentationWidgets,
  withNativePresentationWidgets,
} from '../../../frontend/comfyui/ui/surface/NativePresentationWidgets.js';
import { cubeFacePresentedWidgets } from '../../../frontend/comfyui/ui/surface/CubeFaceWidgetPolicy.js';
import { captureNodeWidgetValues } from '../../../frontend/comfyui/ui/graph/WorkflowWidgetSnapshotCapture.js';

/** Keep only native widget construction/removal outside the real surface ownership under test. */
function fixture() {
  const original: ComfyWidget = { name: 'image', value: 'input.png', type: 'combo' };
  const removed = jest.fn();
  const registered: unknown[] = [];
  const node: ComfyNode = { widgets: [original] };
  node.addCustomWidget = (value: unknown): ComfyWidget => {
    if (!isRecord(value) || typeof value.name !== 'string')
      throw new Error('Invalid native field.');
    registered.push(value.options);
    const widget: ComfyWidget = { ...value, name: value.name, onRemove: removed };
    node.widgets?.push(widget);
    return widget;
  };
  return {
    node,
    original,
    removed,
    registered,
    fields: new NativeNodeFields(node, 'feature', document),
  };
}

test.each(['ready', 'disposed'])(
  'prepares current source choices through the retained native field: %s',
  async (outcome) => {
    const f = fixture();
    const root = document.createElement('article');
    root.className = 'lg-node';
    const row = document.createElement('div');
    row.dataset.scStyleRow = '';
    const trigger = document.createElement('button');
    trigger.setAttribute('role', 'combobox');
    trigger.setAttribute('aria-label', '__sugarcubes_presentation:feature:Source');
    trigger.setAttribute('aria-expanded', 'false');
    row.append(trigger);
    root.append(row);
    document.body.append(root);
    let finish: ((ready: boolean) => void) | undefined;
    const prepare = jest.fn(
      () =>
        new Promise<boolean>((resolve) => {
          finish = resolve;
        }),
    );
    const opened = jest.fn();
    trigger.addEventListener('click', opened);
    const mount = f.fields.mountChoice(root, 'Source', jest.fn(), 'Source', prepare);
    mount.update('composition', [{ value: 'composition', label: 'Composition', blockers: [] }]);
    trigger.click();
    await Promise.resolve();
    expect(prepare).toHaveBeenCalledTimes(1);
    expect(opened).not.toHaveBeenCalled();
    if (outcome === 'disposed') mount.dispose();
    finish?.(true);
    for (let index = 0; index < 12; index++) await Promise.resolve();
    expect(opened).toHaveBeenCalledTimes(outcome === 'ready' ? 1 : 0);
    mount.dispose();
    root.remove();
  },
);

test.each([undefined, 'spinner-slider'] as const)(
  'numeric display %s uses the native lifecycle and remains outside graph serialization',
  (presentation) => {
    const f = fixture(),
      change = jest.fn();
    const mount = f.fields.mountNumber('Feather', change, {
      min: 0,
      max: 512,
      step: 1,
      precision: 0,
      ...(presentation ? { presentation } : {}),
    });
    mount.update(4);
    const widget = nativePresentationWidgets(f.node)[1],
      options = widget?.options;
    expect(widget).toMatchObject({
      type: 'number',
      label: 'Feather',
      value: 4,
      serialize: false,
    });
    expect(options).toMatchObject({
      min: 0,
      max: 512,
      step: 10,
      step2: 1,
      precision: 0,
      ...(presentation ? { presentation } : {}),
      hidden: false,
      disabled: false,
    });
    expect(options).not.toHaveProperty('values');
    widget?.callback?.(9);
    widget?.callback?.('12');
    widget?.callback?.(NaN);
    expect(change.mock.calls).toEqual([[9]]);
    mount.update(12, true, false);
    expect(widget?.options).toBe(options);
    expect(options).not.toHaveProperty('values');
    expect(widget).toMatchObject({ value: 12, disabled: true, hidden: true });
    expect(captureNodeWidgetValues(f.node)).toEqual({ image: 'input.png' });
    mount.dispose();
    expect(nativePresentationWidgets(f.node)).toEqual([f.original]);
    const second = f.fields.mountNumber('Feather', change, {
      min: 0,
      max: 512,
      step: 1,
      precision: 0,
      ...(presentation ? { presentation } : {}),
    });
    second.update(0);
    expect(nativePresentationWidgets(f.node)[1]).toBe(widget);
    expect(widget?.options).toBe(options);
    second.dispose();
  },
);

test('renders native fields through the card widget scope while live graph values and identity stay unchanged', () => {
  const f = fixture(),
    graphWidgets = f.node.widgets;
  const mount = f.fields.mountChoice(document.createElement('article'), 'Source', jest.fn());
  mount.update('composition', [
    { value: 'composition', label: 'Current Composition', blockers: [] },
  ]);
  expect(f.node.widgets).toBe(graphWidgets);
  expect(f.node.widgets).toEqual([f.original]);
  expect(captureNodeWidgetValues(f.node)).toEqual({ image: 'input.png' });
  expect(cubeFacePresentedWidgets(f.node)).toHaveLength(2);
  const projected = withNativePresentationWidgets(f.node, () => [...(f.node.widgets ?? [])]);
  expect(projected[1]?.options).toBe(f.registered[0]);
  expect(projected[1]?.options?.values).toEqual(['composition']);
  expect(projected[1]).toMatchObject({
    type: 'combo',
    label: 'Source',
    value: 'composition',
    serialize: false,
  });
  expect(f.node.widgets).toBe(graphWidgets);
  expect(f.node.widgets).toEqual([f.original]);
  expect(() =>
    withNativePresentationWidgets(f.node, () => {
      throw new Error('Native extraction failed.');
    }),
  ).toThrow('Native extraction failed.');
  expect(f.node.widgets).toEqual([f.original]);
  mount.dispose();
  expect(nativePresentationWidgets(f.node)).toEqual([f.original]);
  expect(f.removed).not.toHaveBeenCalled();
});

test('keeps a read-only action value in the native field row with shared command styling and lifecycle', () => {
  const f = fixture(),
    action = jest.fn(),
    root = document.createElement('article');
  root.className = 'lg-node';
  const field = f.fields.mountActionField(root, 'File', 'Browse…', action);
  field.update('<asset>.png', false, true);
  const widget = nativePresentationWidgets(f.node)[1];
  expect(widget).toMatchObject({
    type: 'string',
    label: 'File',
    value: '<asset>.png',
    serialize: false,
    options: { read_only: true, disabled: false, hidden: false },
  });
  const row = document.createElement('div'),
    layout = document.createElement('div'),
    label = document.createElement('span'),
    fieldValue = document.createElement('div'),
    value = document.createElement('div'),
    input = document.createElement('input');
  const card = document.createElement('article');
  card.className = 'sugarcubes-cube-face__node-card';
  card.append(root);
  document.body.append(card);
  row.dataset.scStyleRow = '';
  layout.dataset.scStyleLayout = '';
  label.dataset.testid = 'widget-layout-field-label';
  label.textContent = 'File';
  input.setAttribute('aria-label', widget?.name ?? '');
  value.append(input);
  fieldValue.append(value);
  layout.append(label, fieldValue);
  row.append(layout);
  root.append(row);
  field.update('<asset>.png', false, true);
  const button = value.querySelector<HTMLButtonElement>('[data-sc-button]');
  expect(button?.textContent).toBe('Browse…');
  expect(getComputedStyle(layout).display).toBe('flex');
  expect(getComputedStyle(label).display).toBe('none');
  expect(getComputedStyle(fieldValue).maxWidth).toBe('none');
  button?.click();
  expect(action).toHaveBeenCalledTimes(1);
  field.update('<asset>.png', true, true);
  button?.click();
  expect(action).toHaveBeenCalledTimes(1);
  expect(value.querySelectorAll('[data-sc-button]')).toHaveLength(1);
  const replacement = document.createElement('div'),
    replacementInput = document.createElement('input');
  replacementInput.setAttribute('aria-label', widget?.name ?? '');
  replacement.append(replacementInput);
  row.replaceChildren(replacement);
  field.update('<asset>.png', false, true);
  expect(value.hasAttribute('data-sc-field-action')).toBe(false);
  expect(replacement.querySelector('[data-sc-button]')).toBe(button);
  expect(root.querySelector('asset')).toBeNull();
  expect(captureNodeWidgetValues(f.node)).toEqual({ image: 'input.png' });
  field.dispose();
  expect(replacement.querySelector('[data-sc-button]')).toBeNull();
  expect(nativePresentationWidgets(f.node)).toEqual([f.original]);
  card.remove();
});

test('shares native field identity across simultaneous views and commits through one active source owner', () => {
  const f = fixture(),
    first = jest.fn(),
    second = jest.fn();
  const one = f.fields.mountChoice(document.createElement('article'), 'Source', first);
  const two = f.fields.mountChoice(document.createElement('article'), 'Source', second);
  one.update('file', [{ value: 'file', label: 'Load From File', blockers: [] }]);
  const widget = nativePresentationWidgets(f.node)[1];
  expect(nativePresentationWidgets(f.node)).toHaveLength(2);
  widget?.callback?.('file');
  expect(first).toHaveBeenCalledTimes(1);
  expect(second).not.toHaveBeenCalled();
  one.dispose();
  widget?.callback?.('file');
  expect(second).toHaveBeenCalledTimes(1);
  expect(f.removed).not.toHaveBeenCalled();
  two.dispose();
  widget?.callback?.('file');
  expect(second).toHaveBeenCalledTimes(1);
  const again = f.fields.mountChoice(document.createElement('article'), 'Source', second);
  again.update('file', [{ value: 'file', label: 'Load From File', blockers: [] }]);
  expect(nativePresentationWidgets(f.node)[1]).toBe(widget);
  again.dispose();
  expect(f.removed).not.toHaveBeenCalled();
});

test('places source choices before their native field anchor while preserving original graph order and identity', () => {
  const f = fixture(),
    channel: ComfyWidget = { name: 'channel', value: 'red', type: 'combo' };
  f.node.widgets!.push(channel);
  const original = f.node.widgets,
    fields = new NativeNodeFields(f.node, 'source', document, 'channel');
  const source = fields.mountChoice(document.createElement('article'), 'Source', jest.fn()),
    layer = fields.mountChoice(document.createElement('article'), 'Layer', jest.fn()),
    entry = fields.mountChoice(document.createElement('article'), 'Entry', jest.fn());
  const projected = nativePresentationWidgets(f.node);
  expect(projected.map((widget) => widget.label ?? widget.name)).toEqual([
    'image',
    'Source',
    'Layer',
    'Entry',
    'channel',
  ]);
  expect(projected.at(-1)).toBe(channel);
  expect(withNativePresentationWidgets(f.node, () => nativePresentationWidgets(f.node))).toEqual(
    projected,
  );
  expect(f.node.widgets).toBe(original);
  expect(f.node.widgets).toEqual([f.original, channel]);
  expect(captureNodeWidgetValues(f.node)).toEqual({ image: 'input.png', channel: 'red' });
  source.dispose();
  layer.dispose();
  entry.dispose();
  expect(nativePresentationWidgets(f.node)).toEqual([f.original, channel]);
});
