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
import { beforeAll, afterAll, afterEach, expect, jest, test } from '@jest/globals';
import { Updates } from '@microsoft/fast-element';
import { SubstituteFrameHost } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameHost.js';
import { AppOrbView } from '../../../frontend/comfyui/ui/surface/frame/AppOrbView.js';
import { ClosedWorkflowHistory } from '../../../frontend/comfyui/ui/surface/frame/ClosedWorkflowHistory.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { WorkflowOutputActivity } from '../../../frontend/comfyui/ui/execution/application/WorkflowOutputActivity.js';
import { workflowSessionId } from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';

const internals = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'attachInternals');
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
  if (internals) Object.defineProperty(HTMLElement.prototype, 'attachInternals', internals);
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

test('the history records only workflows the host closed, newest first, within its budget', () => {
  const history = new ClosedWorkflowHistory();
  const kept = {};
  const closed = {};
  expect(history.available).toBe(false);
  history.capture(
    new Map<object, { title: string; graphJson: string }>([
      [kept, { title: 'Kept', graphJson: '{"nodes":[1]}' }],
      [closed, { title: 'Closed', graphJson: '{"nodes":[2]}' }],
    ]),
  );
  // A close the person cancelled leaves every captured workflow open: nothing is recorded.
  history.settle(new Set([kept, closed]));
  expect(history.available).toBe(false);
  history.settle(new Set([kept]));
  expect(history.take()).toEqual({ title: 'Closed', graphJson: '{"nodes":[2]}' });
  expect(history.take()).toBeNull();
  // The capture was consumed: the same workflow is not recorded twice.
  history.settle(new Set([kept]));
  expect(history.available).toBe(false);
  for (let index = 0; index < 25; index++)
    history.restore({ title: String(index), graphJson: '{}' });
  const titles: string[] = [];
  for (let record = history.take(); record; record = history.take()) titles.push(record.title);
  expect(titles).toHaveLength(20);
  expect(titles[0]).toBe('24');
  expect(titles.at(-1)).toBe('5');
});

/** Mount a frame over a fake Comfy with two open workflows whose native tabs can be closed. */
function fixture() {
  jest.spyOn(AppOrbView.prototype, 'update').mockResolvedValue(undefined);
  const outer = document.createElement('div');
  outer.className = 'workflow-tabs-container';
  const row = document.createElement('div');
  const native = document.createElement('div');
  native.className = 'workflow-tabs-container';
  row.append(native);
  outer.append(row);
  document.body.append(outer);
  const ids = ['first', 'second', 'unread'].map(workflowSessionId);
  const sessions = new WorkflowSessionStore();
  const firstGraph = {
    nodes: [{ id: 1, type: 'First <node>' }],
    extra: { seed: '18446744073709551615' },
  };
  const first = {
    path: 'workflows/first.json',
    filename: 'first',
    isModified: false,
    changeTracker: { activeState: firstGraph },
  };
  const second = { path: 'workflows/second.json', filename: 'second', isModified: false };
  /** A tab Comfy restored in the background: no document until Comfy reads it. */
  const unread: {
    path: string;
    filename: string;
    isModified: boolean;
    content: string | null;
    load: () => Promise<void>;
  } = {
    path: 'workflows/unread.json',
    filename: 'unread',
    isModified: false,
    content: null,
    load: async () => {
      unread.content = '{"nodes":[{"id":3,"type":"Unread"}]}';
    },
  };
  const store = {
    openWorkflows: [first, second, unread] as object[],
    activeWorkflow: second as object,
    // Comfy's store does not announce a closed tab to its subscribers.
    $subscribe: () => () => undefined,
  };
  const title = (workflow: object): string =>
    workflow === first ? 'First' : workflow === second ? 'Second' : 'Unread';
  /** Mirror Comfy's native tabs, which the frame drives for activation and closing. */
  const paint = (): void => {
    native.replaceChildren(
      ...store.openWorkflows.map((workflow) => {
        const tab = document.createElement('div');
        tab.className = 'workflow-tab';
        const label = document.createElement('span');
        label.className = 'workflow-label';
        label.textContent = title(workflow);
        tab.append(label);
        // Comfy closes a tab on a middle-button release.
        tab.addEventListener('mouseup', (event) => {
          if (event.button !== 1 || !closes) return;
          store.openWorkflows = store.openWorkflows.filter((item) => item !== workflow);
          synchronize();
        });
        return tab;
      }),
    );
  };
  const synchronize = (): void => {
    paint();
    sessions.synchronize(
      store.openWorkflows.map((workflow) => ({
        sessionId: ids[workflow === first ? 0 : workflow === second ? 1 : 2]!,
        title: title(workflow),
        path: (workflow as { path: string }).path,
      })),
      ids[1]!,
    );
  };
  let closes = true;
  const loadGraphData = jest.fn(async () => undefined);
  const app = {
    graph: { serialize: () => ({ nodes: [{ id: 2, type: 'Second' }] }) },
    loadGraphData,
    extensionManager: { workflow: store, command: { execute: jest.fn(async () => undefined) } },
    ui: { settings: { addSetting: () => undefined, getSettingValue: () => false } },
  };
  const report = jest.fn();
  const host = new SubstituteFrameHost({
    document,
    app,
    sessions,
    hostColorScheme: { follow: jest.fn(), release: jest.fn() },
    activity: new WorkflowOutputActivity(),
    progress: {
      read: () => ({ visible: false, workflow: 0, sampler: 0 }),
      subscribe: () => () => undefined,
    },
    palette: { theme: { color: '#479ef5', dark: true }, subscribe: () => () => undefined },
    stackControls: () => ({
      available: true,
      action: 'collapse',
      activate: async () => undefined,
      menu: () => [],
      subscribe: () => () => undefined,
    }),
    report,
    openOverrides: () => undefined,
    setOverridesInline: () => undefined,
    adoptJobSeeds: () => 0,
  });
  synchronize();
  host.setup();
  host.mode.setEnabled(true);
  /** Open the context menu at a tab, or on empty tab-bar space, and return its rows. */
  const menu = async (target: Element): Promise<HTMLElement[]> => {
    target.dispatchEvent(
      new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 40, clientY: 10 }),
    );
    await settle();
    await settle();
    return [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];
  };
  return {
    host,
    ids,
    firstGraph,
    loadGraphData,
    report,
    menu,
    refuseClose: () => {
      closes = false;
    },
    tab: (index: number) => document.getElementById(`sc-workflow-${ids[index]!}`)!,
    strip: () => document.getElementById('sc-workflow-strip')!,
  };
}

test('a closed workflow is reopened from the tab bar with its document and title', async () => {
  const f = fixture();
  await settle();
  // Nothing has been closed yet: the action is offered and disabled.
  const idle = await f.menu(f.strip());
  expect(idle.map((row) => row.textContent)).toEqual(['Reopen Closed Workflow']);
  expect(idle[0]?.getAttribute('aria-disabled')).toBe('true');
  idle[0]?.click();
  expect(f.loadGraphData).not.toHaveBeenCalled();
  f.strip().dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));

  const first = f.tab(0);
  first.dispatchEvent(new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true }));
  first.dispatchEvent(new MouseEvent('mouseup', { button: 1, bubbles: true, cancelable: true }));
  await settle();
  expect(document.getElementById(`sc-workflow-${f.ids[0]!}`)).toBeNull();

  const rows = await f.menu(f.strip());
  expect(rows.map((row) => row.textContent)).toEqual(['Reopen Closed Workflow']);
  expect(rows[0]?.getAttribute('aria-disabled')).not.toBe('true');
  rows[0]?.click();
  await settle();
  expect(f.loadGraphData).toHaveBeenCalledTimes(1);
  const [graph, clean, restoreView, title] = f.loadGraphData.mock.calls[0] as unknown as [
    unknown,
    boolean,
    boolean,
    string,
  ];
  expect(graph).toEqual(f.firstGraph);
  expect([clean, restoreView, title]).toEqual([true, true, 'First']);
  // Reopening consumes the record.
  const after = await f.menu(f.strip());
  expect(after[0]?.getAttribute('aria-disabled')).toBe('true');
  expect(f.report).not.toHaveBeenCalled();
  f.host.dispose();
});

test('a close the host did not carry out leaves nothing to reopen', async () => {
  const f = fixture();
  f.refuseClose();
  await settle();
  const first = f.tab(0);
  first.dispatchEvent(new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true }));
  first.dispatchEvent(new MouseEvent('mouseup', { button: 1, bubbles: true, cancelable: true }));
  await settle();
  expect(document.getElementById(`sc-workflow-${f.ids[0]!}`)).not.toBeNull();
  const rows = await f.menu(f.strip());
  expect(rows[0]?.getAttribute('aria-disabled')).toBe('true');
  f.host.dispose();
});

test('a tab Comfy had not read yet is read before it closes, so it too can be reopened', async () => {
  const f = fixture();
  await settle();
  const unread = f.tab(2);
  unread.dispatchEvent(new MouseEvent('mousedown', { button: 1, bubbles: true, cancelable: true }));
  unread.dispatchEvent(new MouseEvent('mouseup', { button: 1, bubbles: true, cancelable: true }));
  await settle();
  await settle();
  expect(document.getElementById(`sc-workflow-${f.ids[2]!}`)).toBeNull();
  const rows = await f.menu(f.strip());
  rows[0]?.click();
  await settle();
  expect(f.loadGraphData).toHaveBeenCalledTimes(1);
  expect(f.loadGraphData.mock.calls[0]).toEqual([
    { nodes: [{ id: 3, type: 'Unread' }] },
    true,
    true,
    'Unread',
  ]);
  expect(f.report).not.toHaveBeenCalled();
  f.host.dispose();
});
