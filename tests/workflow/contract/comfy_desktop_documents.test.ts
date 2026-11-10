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
  documents.install();
  return {
    workflow,
    graphLoads: () => graphLoads,
    app,
    sessions,
    adapter,
    documents,
    dispose: () => {
      documents.dispose();
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

test('materializes named and temporary dirty graphs in order and retains each saved baseline after later activation', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', false));
  const b = entry('00000000-0000-4000-8000-000000000002', document('B', true));
  const bindings = await host.documents.materialize([a, b], a.id, () => true);
  expect(host.workflow.openWorkflows.map((item) => item.filename)).toEqual(['A', 'B']);
  expect(host.workflow.activeWorkflow?.filename).toBe('A');
  expect(host.workflow.openWorkflows[0].isTemporary).toBe(false);
  expect(host.workflow.openWorkflows[1].isTemporary).toBe(true);
  const context = bindings.get(a.id)!;
  expect(host.documents.capture(context, 3)).toEqual(a.document);
  const target = host.workflow.openWorkflows[1];
  await host.app.loadGraphData(JSON.parse(b.document.graphJson), true, true, target);
  expect(target.changeTracker?.initialState).toEqual(JSON.parse(b.document.baselineGraphJson!));
  expect(target.isModified).toBe(true);
  host.dispose();
});

test('restoring several tabs loads only the active workflow graph and still opens every tab in order', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', false));
  const b = entry('00000000-0000-4000-8000-000000000002', document('B', true));
  const c = entry('00000000-0000-4000-8000-000000000003', document('C', false));
  await host.documents.materialize([a, b, c], b.id, () => true);
  expect(host.graphLoads()).toBe(1);
  expect(host.workflow.openWorkflows.map((item) => item.filename)).toEqual(['A', 'B', 'C']);
  expect(host.workflow.activeWorkflow?.filename).toBe('B');
  expect(host.workflow.openWorkflows.map((item) => item.changeTracker?.initialState)).toEqual(
    [a, b, c].map((item) => JSON.parse(item.document.baselineGraphJson ?? '')),
  );
  host.dispose();
});

test('captures inactive tracker and unloaded content instead of assigning active graph to all documents', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', false));
  const b = entry('00000000-0000-4000-8000-000000000002', document('B', true));
  const bindings = await host.documents.materialize([a, b], a.id, () => true);
  const inactive = host.workflow.openWorkflows[1];
  const bContext = bindings.get(b.id)!;
  expect(host.documents.capture(bContext, 3).graphJson).toBe(b.document.graphJson);
  inactive.changeTracker = null;
  expect(host.documents.capture(bContext, 3).graphJson).toBe(b.document.graphJson);
  host.dispose();
});

test('captures Comfy optional serialized fields using its normal JSON representation without mutating host state', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  const contexts = await host.documents.materialize([a], a.id, () => true);
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
  const contexts = await host.documents.materialize([a], a.id, () => true);
  host.app.rootGraph.serialize = () => ({ nodes: [], extra: { invalid: value } });
  expect(() => host.documents.capture(contexts.get(a.id)!, 3)).toThrow(TypeError);
  host.dispose();
});

test('refuses unpaired property names before JSON can escape them', async () => {
  const host = fixture();
  const a = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  const contexts = await host.documents.materialize([a], a.id, () => true);
  host.app.rootGraph.serialize = () => ({ nodes: [], extra: { ['\ud800']: 'invalid' } });
  expect(() => host.documents.capture(contexts.get(a.id)!, 3)).toThrow(TypeError);
  host.dispose();
});

test('invalid document or superseded restore performs no admitted document replacement', async () => {
  const host = fixture();
  const valid = entry('00000000-0000-4000-8000-000000000001', document('A', true));
  await expect(
    host.documents.materialize(
      [{ ...valid, document: { ...valid.document, graphJson: 'invalid' } }],
      valid.id,
      () => true,
    ),
  ).rejects.toThrow('admission');
  expect(host.workflow.openWorkflows).toEqual([]);
  await expect(host.documents.materialize([], null, () => true)).rejects.toThrow('Comfy default');
  await expect(host.documents.materialize([valid], valid.id, () => false)).rejects.toThrow(
    'superseded',
  );
  expect(host.workflow.openWorkflows).toEqual([]);
  host.dispose();
});
