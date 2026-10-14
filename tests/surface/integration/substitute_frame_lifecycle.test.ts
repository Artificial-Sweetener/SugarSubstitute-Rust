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
/** Prove mode restoration and stable official Fluent tab lifecycles without changing graph state. */
import { beforeAll, afterAll, afterEach, expect, jest, test } from '@jest/globals';
import { Updates } from '@microsoft/fast-element';
import { SubstituteFrameHost } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameHost.js';
import { WorkflowTabsView } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabsView.js';
import { AppOrbView } from '../../../frontend/comfyui/ui/surface/frame/AppOrbView.js';
import { SubstituteFrameView } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameView.js';
import { ContextMenu } from '../../../frontend/comfyui/ui/menus/ContextMenu.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowOutputActivity } from '../../../frontend/comfyui/ui/execution/application/WorkflowOutputActivity.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

const internalsDescriptor = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  'attachInternals',
);
beforeAll(() => {
  // JSDOM lacks shadow-root internals; preserve Microsoft's actual bindings and focus group.
  Object.defineProperty(HTMLElement.prototype, 'attachInternals', {
    configurable: true,
    value: function (this: HTMLElement) {
      const host = this;
      return {
        role: null,
        ariaDisabled: null,
        states: new Set<string>(),
        get shadowRoot() {
          return host.shadowRoot;
        },
      };
    },
  });
});
afterAll(() => {
  if (internalsDescriptor)
    Object.defineProperty(HTMLElement.prototype, 'attachInternals', internalsDescriptor);
  else Reflect.deleteProperty(HTMLElement.prototype, 'attachInternals');
});

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.removeAttribute('data-sc-substitute-frame');
  jest.restoreAllMocks();
});

/** Flush Microsoft bindings and host surface delivery without elapsed-time sleeps. */
async function settle(): Promise<void> {
  await Updates.next();
  await Promise.resolve();
  await Updates.next();
}

test('mode restores host presentation and preserves native controls, graph, and renderer choice', async () => {
  jest.spyOn(AppOrbView.prototype, 'update').mockResolvedValue(undefined);
  const outer = document.createElement('div');
  outer.className = 'workflow-tabs-container';
  const row = document.createElement('div');
  const native = document.createElement('div');
  native.className = 'workflow-tabs-container';
  row.append(native);
  outer.append(row);
  document.body.append(outer);
  const a = workflowSessionId('a');
  const sessions = new WorkflowSessionStore();
  sessions.synchronize([{ sessionId: a, title: 'A', path: 'a' }], a);
  const workflow = { path: 'a', isModified: false };
  const store = {
    openWorkflows: [workflow],
    activeWorkflow: workflow,
    $subscribe: () => () => undefined,
  };
  const graph = { nodes: ['unchanged'] };
  const app = {
    graph,
    extensionManager: { workflow: store },
    ui: { settings: { addSetting: () => undefined, getSettingValue: () => false } },
  };
  const report = jest.fn();
  const host = new SubstituteFrameHost({
    document,
    app,
    sessions,
    activity: new WorkflowOutputActivity(),
    palette: { theme: { color: '#479ef5', dark: true }, subscribe: () => () => undefined },
    stackControls: () => ({
      compact: false,
      toggleCompact: async () => undefined,
      menu: () => [],
      subscribe: () => () => undefined,
    }),
    report,
    openOverrides: () => undefined,
  });
  host.setup();
  expect(document.querySelector('.sc-substitute-frame')).toBeNull();
  host.mode.setEnabled(true);
  await settle();
  expect(row.hasAttribute('data-sc-frame-native')).toBe(true);
  const frame = document.querySelector('.sc-substitute-frame');
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  await settle();
  expect(document.querySelector('.sc-substitute-frame')).toBe(frame);
  expect(app.graph).toBe(graph);
  host.mode.setEnabled(false);
  await settle();
  expect(row.hasAttribute('data-sc-frame-native')).toBe(false);
  expect(native.parentElement).toBe(row);
  expect(document.querySelector('.sc-substitute-frame')).toBeNull();
  expect(document.documentElement.hasAttribute('data-sc-substitute-frame')).toBe(false);
  expect(report).not.toHaveBeenCalled();
  host.dispose();
});

test('literal titles and keyed tabs survive reorder and activity refresh without replaying shimmer', async () => {
  const a = workflowSessionId('a');
  const b = workflowSessionId('b');
  const actions = {
    activate: jest.fn(),
    close: jest.fn(),
    reorder: jest.fn(),
    create: jest.fn(),
    report: jest.fn(),
  };
  const view = new WorkflowTabsView(document, actions);
  document.body.append(view.element);
  const tabs = [
    { id: a, title: '<img src=x onerror=bad()>', modified: false, selected: true },
    { id: b, title: 'B', modified: false, selected: false },
  ];
  view.update(tabs, (id) => id === b);
  await settle();
  const first = document.getElementById(`sc-workflow-${a}`)!;
  const unread = document.getElementById(`sc-workflow-${b}`)!;
  expect(first.querySelector('img')).toBeNull();
  expect(first.textContent).toContain(tabs[0]!.title);
  expect(view.element.style.getPropertyValue('--sc-selected-offset')).toBe('0px');
  expect(unread.hasAttribute('data-shimmer')).toBe(true);
  unread.dispatchEvent(new Event('animationend'));
  view.update([...tabs].reverse(), (id) => id === b);
  await settle();
  expect(document.getElementById(`sc-workflow-${a}`)).toBe(first);
  expect(view.element.style.getPropertyValue('--sc-selected-offset')).toBe('180px');
  expect(unread.hasAttribute('data-unread')).toBe(true);
  expect(unread.hasAttribute('data-shimmer')).toBe(false);
  first.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowLeft', altKey: true, bubbles: true }),
  );
  expect(actions.reorder).toHaveBeenCalledWith(1, 0);
  view.dispose();
});

test('frame actions retain stack policy, settings access, override anchor and unread mount state', async () => {
  jest.spyOn(AppOrbView.prototype, 'update').mockResolvedValue(undefined);
  const command = jest.fn();
  const openOverrides = jest.fn();
  let compact = true;
  let refresh = (): void => undefined;
  const toggleCompact = jest.fn(async () => {
    compact = !compact;
    refresh();
  });
  const stop = jest.fn();
  const view = new SubstituteFrameView(document, {
    workflows: {
      activate: jest.fn(),
      close: jest.fn(),
      reorder: jest.fn(),
      create: jest.fn(),
      report: jest.fn(),
    },
    command,
    openOverrides,
    report: jest.fn(),
    stack: {
      get compact() {
        return compact;
      },
      toggleCompact,
      menu: () => [{ label: 'Show Cube stack', checked: true }],
      subscribe: (listener) => {
        refresh = listener;
        return stop;
      },
    },
  });
  document.body.append(view.element);
  const stack = view.element.querySelector<HTMLElement>('[aria-label="Expand Cube stack"]')!;
  stack.click();
  await settle();
  expect(toggleCompact).toHaveBeenCalledTimes(1);
  expect(stack.getAttribute('aria-label')).toBe('Collapse Cube stack');
  const open = jest.spyOn(ContextMenu.prototype, 'open');
  stack.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
  expect(open.mock.calls.at(-1)?.[0].entries[0]?.label).toBe('Show Cube stack');
  const pin = view.element.querySelector<HTMLElement>('[aria-label="Set Global Override"]')!;
  pin.click();
  expect(openOverrides).toHaveBeenCalledWith(pin);
  view.element.querySelector<HTMLElement>('.sc-app-orb')!.click();
  const settings = open.mock.calls.at(-1)?.[0].entries.find((entry) => entry.label === 'Settings');
  expect(settings?.separator).not.toBe(true);
  settings?.select?.();
  expect(command).toHaveBeenCalledWith('Comfy.ShowSettingsDialog');
  const id = workflowSessionId('unread');
  const tabs = [{ id, title: 'Unread', modified: false, selected: false }];
  view.update(tabs, () => true);
  await settle();
  const tab = document.getElementById(`sc-workflow-${id}`)!;
  expect(tab.hasAttribute('data-unread')).toBe(true);
  expect(tab.hasAttribute('data-shimmer')).toBe(false);
  view.update(tabs, () => false);
  view.update(tabs, () => true);
  expect(tab.hasAttribute('data-shimmer')).toBe(true);
  view.dispose();
  expect(stop).toHaveBeenCalledTimes(1);
});

test('overflow activation reveals the selected tab and activity preserves deliberate scrolling', async () => {
  const ids = ['a', 'b', 'c'].map(workflowSessionId);
  const view = new WorkflowTabsView(document, {
    activate: jest.fn(),
    close: jest.fn(),
    reorder: jest.fn(),
    create: jest.fn(),
    report: jest.fn(),
  });
  document.body.append(view.element);
  Object.defineProperty(view.element, 'clientWidth', { configurable: true, value: 280 });
  const tabs = ids.map((id, index) => ({
    id,
    title: String(index),
    modified: false,
    selected: index === 2,
  }));
  view.update(tabs, () => false);
  await settle();
  expect(view.element.scrollLeft).toBe(260);
  view.element.scrollLeft = 0;
  view.update(tabs, () => true);
  expect(view.element.scrollLeft).toBe(0);
  view.update(
    tabs.map((tab, index) => ({ ...tab, selected: index === 1 })),
    () => false,
  );
  expect(view.element.scrollLeft).toBe(80);
  view.update(
    tabs.map((tab, index) => ({ ...tab, selected: index === 0 })),
    () => false,
  );
  expect(view.element.scrollLeft).toBe(0);
  view.dispose();
});
