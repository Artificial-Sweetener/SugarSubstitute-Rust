/** @jest-environment node */
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
/** Exercise deployed-shaped Comfy document operations without replacing their semantic owner. */
import { ComfyDesktopDocuments } from '../../../frontend/comfyui/ui/workflow/infrastructure/ComfyDesktopDocuments.js';
import { ComfyWorkflowSessionAdapter } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSessionAdapter.js';
import { WorkflowSessionStore } from '../../../frontend/comfyui/ui/execution/application/WorkflowSessionStore.js';
import { createComfyWorkflowLoadObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowLoadObserver.js';
import type {
  SessionDocument,
  SessionEntry,
} from '../../../integrations/contracts/SessionProtocol.generated.js';

interface HostWorkflow {
  path: string;
  filename: string;
  size: number;
  readonly isTemporary: boolean;
  isModified: boolean;
  content: string;
  originalContent: string;
  changeTracker: {
    activeState: object;
    initialState: object;
    ds: { scale: number; offset: number[] };
  } | null;
  updatePath(path: string): void;
  load(): Promise<HostWorkflow>;
}

/** Model the exposed workflow store and actual load reset/baseline behavior at the Comfy boundary. */
function fixture() {
  let graph: object = { nodes: [], extra: { title: 'active' } };
  let key = 0;
  let graphLoads = 0;
  const listeners = new Set<() => void>();
  const workflow = {
    openWorkflows: [] as HostWorkflow[],
    activeWorkflow: null as HostWorkflow | null,
    $subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    createNewTemporary(name: string, state: object): HostWorkflow {
      return {
        path: `workflows/${name}-${++key}.json`,
        filename: name,
        size: -1,
        get isTemporary() {
          return this.size === -1;
        },
        isModified: false,
        content: JSON.stringify(state),
        originalContent: JSON.stringify(state),
        changeTracker: null,
        updatePath(path: string) {
          this.path = path;
        },
        /** Build the change tracker from the workflow's own content, as Comfy's load does. */
        async load() {
          this.changeTracker ??= {
            activeState: JSON.parse(this.content),
            initialState: JSON.parse(this.originalContent),
            ds: { scale: 1, offset: [0, 0] },
          };
          return this;
        },
      };
    },
    async closeWorkflow(target: HostWorkflow) {
      this.openWorkflows = this.openWorkflows.filter((item) => item !== target);
      for (const listener of listeners) listener();
    },
    attachWorkflow(target: HostWorkflow, at: number) {
      this.openWorkflows.splice(at, 0, target);
      for (const listener of listeners) listener();
    },
    reorderWorkflows(from: number, to: number) {
      const [target] = this.openWorkflows.splice(from, 1);
      this.openWorkflows.splice(to, 0, target);
    },
  };
  const app = {
    extensionManager: { workflow },
    canvas: { ds: { scale: 1, offset: [0, 0] } },
    rootGraph: { serialize: () => structuredClone(graph) },
    async loadGraphData(state: object, _clean: boolean, _restore: boolean, target: HostWorkflow) {
      graphLoads++;
      graph = structuredClone(state);
      if (!workflow.openWorkflows.includes(target)) workflow.openWorkflows.push(target);
      workflow.activeWorkflow = target;
      target.changeTracker ??= {
        activeState: state,
        initialState: state,
        ds: { scale: 1, offset: [0, 0] },
      };
      target.changeTracker.activeState = structuredClone(state);
      target.changeTracker.initialState = structuredClone(state);
      for (const listener of listeners) listener();
    },
  };
  const sessions = new WorkflowSessionStore();
  const adapter = new ComfyWorkflowSessionAdapter({
    app,
    sessions,
    nextKey: () => `runtime-${++key}`,
    logger: { error: () => {} },
  });
  adapter.install();
  const loads = createComfyWorkflowLoadObserver(app, sessions, adapter, { error: () => {} });
  loads.install();
  const documents = new ComfyDesktopDocuments({ app, sessions, adapter });
  return {
    workflow,
    graphLoads: () => graphLoads,
    app,
    sessions,
    adapter,
    documents,
    dispose: () => {
      loads.dispose();
      adapter.dispose();
      sessions.dispose();
    },
  };
}

/** Supply valid baseline/current graphs and independent graph-view intent. */
function document(name: string, temporary: boolean): SessionDocument {
  return {
    graphJson: JSON.stringify({ nodes: [], extra: { title: `${name} current` } }),
    baselineGraphJson: JSON.stringify({ nodes: [], extra: { title: `${name} saved` } }),
    graphRevision: 3,
    path: temporary ? null : `workflows/${name}.json`,
    name,
    temporary,
    dirty: true,
    sourceIdentity: temporary ? null : `workflows/${name}.json`,
    view: { x: -40, y: 25, scale: 2 },
  };
}

/** Documents are the only fields consumed by this host adapter. */
function entry(id: string, value: SessionDocument): SessionEntry {
  return {
    id,
    document: value,
    input: null,
    sourceRevision: 0,
    navigationRevision: 0,
    sources: [],
    outputs: [],
    submissions: [],
    view: {
      canvasEnabled: true,
      masonryEnabled: false,
      editorWidth: 0,
      route: 'output',
      follow: 'auto',
      mode: 'image',
      linked: true,
      selection: null,
      comparison: { enabled: false, selection: null, orientation: 'vertical', split: 0.5 },
      cameras: [],
      navigation: {
        sceneOverview: false,
        sceneId: null,
        sourceId: null,
        batchIndex: 0,
        batchOverview: false,
      },
      sourcePreview: true,
      sharedInspection: null,
    },
  };
}

/** Reopen tabs the way Comfy restores them from its own persistence, then show the active one. */
async function reopen(
  host: ReturnType<typeof fixture>,
  documents: readonly SessionDocument[],
  active: number,
): Promise<HostWorkflow[]> {
  const reopened = documents.map((value, index) => {
    const created = host.workflow.createNewTemporary(value.name, JSON.parse(value.graphJson));
    if (value.temporary) created.path = `workflows/${value.name}.json`;
    else {
      created.path = value.path!;
      created.size = value.graphJson.length;
    }
    created.originalContent = value.baselineGraphJson ?? value.graphJson;
    host.workflow.attachWorkflow(created, index);
    return created;
  });
  const shown = reopened[active]!;
  await host.app.loadGraphData(JSON.parse(documents[active]!.graphJson), true, true, shown);
  return reopened;
}

test('matches checkpoint entries to the tabs Comfy reopened without opening, closing or loading any', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', false));
  const b = entry('00000000-0000-4000-8000-000000000002', document('B', true));
  const c = entry('00000000-0000-4000-8000-000000000003', document('C', false));
  const unsaved = document('Comfy only', true);
  const tabs = await reopen(host, [b.document, unsaved, a.document], 2);
  const loads = host.graphLoads();
  const contexts = host.documents.match([a, b, c], () => true);
  expect(host.workflow.openWorkflows).toEqual(tabs);
  expect(host.workflow.activeWorkflow).toBe(tabs[2]);
  expect(host.graphLoads()).toBe(loads);
  expect([...contexts.keys()]).toEqual([a.id, b.id]);
  expect(host.adapter.readHostWorkflow(contexts.get(a.id)!.sessionId)).toBe(tabs[2]);
  expect(host.adapter.readHostWorkflow(contexts.get(b.id)!.sessionId)).toBe(tabs[0]);
  host.dispose();
});

test('a checkpoint naming none of the reopened tabs leaves every Comfy tab as Comfy restored it', async () => {
  const host = fixture();
  const tabs = await reopen(host, [document('A', false), document('B', true)], 1);
  const stale = entry('00000000-0000-4000-8000-000000000001', document('Unsaved', true));
  expect(host.documents.match([stale], () => true).size).toBe(0);
  expect(host.documents.match([], () => true).size).toBe(0);
  expect(host.workflow.openWorkflows).toEqual(tabs);
  expect(host.workflow.activeWorkflow).toBe(tabs[1]);
  host.dispose();
});

test('captures inactive tracker and unloaded content instead of assigning active graph to all documents', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', false));
  const b = entry('00000000-0000-4000-8000-000000000002', document('B', true));
  const [, inactive] = await reopen(host, [a.document, b.document], 0);
  await inactive!.load();
  const bContext = host.documents.match([a, b], () => true).get(b.id)!;
  expect(host.documents.capture(bContext, 3).graphJson).toBe(b.document.graphJson);
  inactive!.changeTracker = null;
  expect(host.documents.capture(bContext, 3).graphJson).toBe(b.document.graphJson);
  host.dispose();
});

test('a matched tab keeps its checkpoint source identity before and after Comfy reads its file', async () => {
  const host = fixture();
  const renamed = (name: string): SessionDocument => ({
    ...document(name, false),
    sourceIdentity: `workflows/${name} before rename.json`,
  });
  const a = entry('00000000-0000-4000-8000-000000000001', renamed('A'));
  const b = entry('00000000-0000-4000-8000-000000000002', renamed('B'));
  const [, inactive] = await reopen(host, [a.document, b.document], 0);
  const contexts = host.documents.match([a, b], () => true);
  expect(host.documents.capture(contexts.get(a.id)!, 3).sourceIdentity).toBe(
    a.document.sourceIdentity,
  );
  expect(inactive!.changeTracker).toBeNull();
  expect(host.documents.capture(contexts.get(b.id)!, 3).sourceIdentity).toBe(
    b.document.sourceIdentity,
  );
  await inactive!.load();
  expect(host.documents.capture(contexts.get(b.id)!, 3).sourceIdentity).toBe(
    b.document.sourceIdentity,
  );
  host.dispose();
});

test('captures Comfy optional serialized fields using its normal JSON representation without mutating host state', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  await reopen(host, [a.document], 0);
  const contexts = host.documents.match([a], () => true);
  const graph = {
    nodes: [{ id: 1, widgets_values: [undefined, 'kept'] }],
    links: [],
    floatingLinks: undefined,
    definitions: undefined,
    extra: { reroutes: undefined, title: 'current' },
  };
  const baseline = { nodes: [], definitions: undefined, extra: { reroutes: undefined } };
  host.app.rootGraph.serialize = () => graph;
  const tracker = host.workflow.activeWorkflow?.changeTracker;
  if (!tracker) throw new Error('Committed host tracker is unavailable');
  tracker.initialState = baseline;
  const captured = host.documents.capture(contexts.get(a.id)!, 3);
  expect(captured.graphJson).toBe(JSON.stringify(graph));
  expect(captured.baselineGraphJson).toBe(JSON.stringify(baseline));
  expect(JSON.parse(captured.graphJson).nodes[0].widgets_values).toEqual([null, 'kept']);
  expect(Object.hasOwn(graph, 'definitions')).toBe(true);
  expect(Object.hasOwn(graph.extra, 'reroutes')).toBe(true);
  expect(Object.hasOwn(baseline, 'definitions')).toBe(true);
  host.dispose();
});

test.each([
  { name: 'NaN', value: NaN },
  { name: 'positive infinity', value: Infinity },
  { name: 'negative infinity', value: -Infinity },
  { name: 'unpaired Unicode', value: '\ud800' },
  { name: 'function', value: () => 'not data' },
  { name: 'symbol', value: Symbol('not data') },
  { name: 'bigint', value: 1n },
])('refuses lossy Comfy snapshot value $name', async ({ value }) => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  await reopen(host, [a.document], 0);
  const contexts = host.documents.match([a], () => true);
  host.app.rootGraph.serialize = () => ({ nodes: [], extra: { invalid: value } });
  expect(() => host.documents.capture(contexts.get(a.id)!, 3)).toThrow(TypeError);
  host.dispose();
});

test('refuses unpaired property names before JSON can escape them', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  await reopen(host, [a.document], 0);
  const contexts = host.documents.match([a], () => true);
  host.app.rootGraph.serialize = () => ({ nodes: [], extra: { ['\ud800']: 'invalid' } });
  expect(() => host.documents.capture(contexts.get(a.id)!, 3)).toThrow(TypeError);
  host.dispose();
});

test('an invalid checkpoint document or a superseded restore matches no tab', async () => {
  const host = fixture();
  const valid = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  const tabs = await reopen(host, [valid.document], 0);
  expect(() =>
    host.documents.match(
      [{ ...valid, document: { ...valid.document, graphJson: 'invalid' } }],
      () => true,
    ),
  ).toThrow('admission');
  expect(() => host.documents.match([valid], () => false)).toThrow('superseded');
  expect(host.workflow.openWorkflows).toEqual(tabs);
  host.dispose();
});
