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
/** Prove lossless native action projection, lazy-portal cleanup and failed-read recovery. */
import { jest } from '@jest/globals';
import { ComfyWorkflowTabMenuAdapter } from '../../../frontend/comfyui/ui/surface/frame/ComfyWorkflowTabMenuAdapter.js';

beforeEach(() => {
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    queueMicrotask(() => callback(0));
    return 0;
  });
});

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute('data-sc-tab-menu-capture');
  jest.restoreAllMocks();
});

/** Publish a scoped native action list only while its real controller is open. */
function mount(initial: readonly unknown[], deferSurface = false) {
  let items = initial;
  const tab = document.createElement('div');
  const app = document.createElement('div');
  app.id = 'vue-app';
  app.append(tab);
  document.body.append(app);
  const surface = document.createElement('div');
  const focusScope = document.createElement('div');
  focusScope.setAttribute('role', 'menu');
  surface.append(focusScope);
  const cleanup: boolean[] = [];
  const tree: Record<string, unknown> = { children: [] };
  const setOpen = jest.fn((open: boolean) => {
    expect(document.documentElement.hasAttribute('data-sc-tab-menu-capture')).toBe(true);
    if (open) {
      app.append(surface);
      const content: Record<string, unknown> = {
        type: { __name: 'ContextMenuContent' },
        children: [{ type: { __name: 'WorkflowActionsList' }, props: { items } }],
      };
      tree.children = [content];
      if (deferSurface)
        queueMicrotask(() =>
          queueMicrotask(() => {
            content.el = surface;
          }),
        );
      else content.el = surface;
    }
    if (!open) {
      surface.remove();
      tree.children = [];
      window.setTimeout(() => cleanup.push(true), 0);
    }
  });
  Object.defineProperty(app, '_vnode', {
    value: {
      type: { __name: 'WorkflowTab' },
      component: {
        props: { workflowOption: { workflow: { path: 'target' } } },
        subTree: {
          type: { __name: 'ContextMenuRoot' },
          component: {
            provides: { [Symbol('ContextMenuRootContext')]: { onOpenChange: setOpen } },
            subTree: tree,
          },
        },
      },
    },
  });
  const adapter = new ComfyWorkflowTabMenuAdapter(document);
  const report = jest.fn();
  return {
    tab,
    report,
    setOpen,
    surface,
    cleanup,
    read: () => adapter.read(tab, 'target', report),
    replace: (next: readonly unknown[]) => {
      items = next;
    },
  };
}

test('every published host command retains order, labels and conditional visibility', async () => {
  const ids = [
    'rename',
    'duplicate',
    'toggle-bookmark',
    'save',
    'save-as',
    'export',
    'export-api',
    'share',
    'enter-app-mode',
    'exit-app-mode',
    'enter-builder-mode',
    'clear-workflow',
    'publish',
    'close-tab',
    'close-tabs-to-left',
    'close-tabs-to-right',
    'close-other-tabs',
  ];
  const command = jest.fn();
  const f = mount([
    { separator: true },
    ...ids.map((id) => ({ id, label: `Translated ${id}`, command })),
    { label: 'Hidden', visible: false, command },
    { separator: true },
  ]);
  const entries = await f.read();
  expect(entries.map((entry) => entry.attributes?.['data-sc-native-tab-action'])).toEqual(ids);
  expect(entries.map((entry) => entry.label)).toEqual(ids.map((id) => `Translated ${id}`));
  for (const entry of entries) entry.select?.();
  expect(command).toHaveBeenCalledTimes(ids.length);
  expect(f.setOpen.mock.calls).toEqual([[true], [false]]);
  expect(document.documentElement.hasAttribute('data-sc-tab-menu-capture')).toBe(false);
});

test('native disabled actions stay disabled and markup-like titles and icons remain literal', async () => {
  const command = jest.fn();
  const f = mount([
    { label: '<img src=x onerror=bad()>', icon: '<svg>', command, disabled: true },
    { separator: true },
    { visible: false, label: 'Hidden', command },
    { separator: true },
    {
      label: 'Close Tabs to Right',
      command,
      overlayIcon: { mainIcon: 'pi pi-times', subIcon: 'pi pi-arrow-right', subIconScale: 0.5 },
    },
  ]);
  const entries = await f.read();
  expect(entries.map((entry) => entry.label)).toEqual([
    '<img src=x onerror=bad()>',
    '',
    'Close Tabs to Right',
  ]);
  expect(entries[0]?.disabled).toBe(true);
  entries[0]?.select?.();
  expect(command).not.toHaveBeenCalled();
  const literal = entries[0]?.icon?.(document);
  expect(literal?.className).toBe('<svg>');
  expect(literal?.childElementCount).toBe(0);
  const composite = entries[2]?.icon?.(document);
  expect(composite?.querySelector('.pi-times')).not.toBeNull();
  expect(composite?.querySelector<HTMLElement>('.pi-arrow-right')?.style.fontSize).toBe('50%');
});

test('malformed host actions close the portal and do not poison subsequent menu requests', async () => {
  const f = mount([{ label: 'Invalid' }]);
  await expect(f.read()).rejects.toThrow('missing its label or command');
  expect(f.setOpen.mock.calls).toEqual([[true], [false]]);
  expect(document.documentElement.hasAttribute('data-sc-tab-menu-capture')).toBe(false);
  f.replace([{ label: 'Recovered', command: jest.fn() }]);
  await expect(f.read()).resolves.toEqual([expect.objectContaining({ label: 'Recovered' })]);
});

test('asynchronous command failures are reported and detached native targets cannot execute', async () => {
  const failure = new Error('native save failed');
  const command = jest.fn(async () => {
    throw failure;
  });
  const f = mount([{ label: 'Save', command }]);
  const entries = await f.read();
  entries[0]?.select?.();
  await Promise.resolve();
  expect(f.report).toHaveBeenCalledWith(failure);
  f.tab.remove();
  entries[0]?.select?.();
  expect(command).toHaveBeenCalledTimes(1);
  expect(f.report).toHaveBeenLastCalledWith(
    expect.objectContaining({ message: expect.stringContaining('was replaced') }),
  );
});

test('concurrent requests serialize their native portal lifetimes', async () => {
  const f = mount([{ label: 'Save', command: jest.fn() }]);
  await Promise.all([f.read(), f.read()]);
  expect(f.setOpen.mock.calls).toEqual([[true], [false], [true], [false]]);
});

test('native portal cleanup finishes before action handoff and capture state is released', async () => {
  const f = mount([{ label: 'Save', command: jest.fn() }]);
  await f.read();
  expect(f.cleanup).toEqual([true]);
  expect(document.documentElement.hasAttribute('data-sc-tab-menu-capture')).toBe(false);
});

test('computed actions arriving before their native element still wait for portal cleanup', async () => {
  const f = mount([{ label: 'Save', command: jest.fn() }], true);
  await f.read();
  expect(f.cleanup).toEqual([true]);
  expect(window.requestAnimationFrame).toHaveBeenCalledTimes(2);
});

test('missing host menu controllers fail closed without hiding existing menus', async () => {
  const f = mount([]);
  await expect(
    new ComfyWorkflowTabMenuAdapter(document).read(f.tab, 'missing', f.report),
  ).rejects.toThrow('controls are unavailable');
  expect(f.setOpen).not.toHaveBeenCalled();
  expect(document.documentElement.hasAttribute('data-sc-tab-menu-capture')).toBe(false);
});

test.each([{ disabled: 'true' }, { visible: 'true' }])(
  'malformed availability fails closed: %p',
  async (availability) => {
    const command = jest.fn();
    const f = mount([{ label: 'Save', command, ...availability }]);
    await expect(f.read()).rejects.toThrow('availability is malformed');
    expect(command).not.toHaveBeenCalled();
    expect(f.setOpen.mock.calls).toEqual([[true], [false]]);
    expect(document.documentElement.hasAttribute('data-sc-tab-menu-capture')).toBe(false);
  },
);
