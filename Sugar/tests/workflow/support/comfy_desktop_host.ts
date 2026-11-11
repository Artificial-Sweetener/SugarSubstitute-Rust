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
/** Model the exposed deployed workflow, startup and graph-load boundaries. */
export interface HostWorkflow {
  path: string;
  filename: string;
  size: number;
  readonly isTemporary: boolean;
  readonly isLoaded: boolean;
  isLoading: boolean;
  isModified: boolean;
  content: string | null;
  originalContent: string | null;
  changeTracker: {
    activeState: object;
    initialState: object;
    ds: { scale: number; offset: number[] };
  } | null;
  updatePath(path: string): void;
  load(): Promise<HostWorkflow>;
  unload(): void;
}

/** Model the exposed workflow store and actual load reset/baseline behavior at the Comfy boundary. */
export function comfyDesktopHostBoundary() {
  let graph: object = { nodes: [], extra: { title: 'active' } };
  let key = 0;
  const workspaceListeners = new Set<() => void>();
  const listeners = new Set<() => void>();
  const files = new Map<string, string>();
  let loadGate: Promise<void> = Promise.resolve();
  let trackerGate: Promise<void> = Promise.resolve();
  const failingLoads = new Set<string>();
  const loadedPaths: string[] = [];
  const workflow = {
    openWorkflows: [] as HostWorkflow[],
    activeWorkflow: null as HostWorkflow | null,
    $subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    /** Model a workflow the store lists from the user's saved files without having read its content. */
    createSaved(path: string): HostWorkflow {
      const content = files.get(path);
      if (content === undefined) throw new Error(`Missing saved workflow file: ${path}`);
      return hostWorkflow(
        path,
        path.replace(/^workflows\//, '').replace(/\.json$/, ''),
        content.length,
      );
    },
    createNewTemporary(name: string, state: object): HostWorkflow {
      const created = hostWorkflow(`workflows/${name}-${++key}.json`, name, -1);
      created.content = JSON.stringify(state);
      created.originalContent = created.content;
      return created;
    },
    async closeWorkflow(target: HostWorkflow) {
      this.openWorkflows = this.openWorkflows.filter((item) => item !== target);
      if (!target.isTemporary) target.unload();
      for (const listener of listeners) listener();
    },
    attachWorkflow(target: HostWorkflow, at: number) {
      this.openWorkflows.splice(at, 0, target);
      for (const listener of listeners) listener();
    },
    reorderWorkflows(from: number, to: number) {
      const [target] = this.openWorkflows.splice(from, 1);
      if (!target) throw new Error('Workflow reorder source is unavailable.');
      this.openWorkflows.splice(to, 0, target);
    },
  };
  const app = {
    extensionManager: {
      workflow,
      spinner: true,
      $subscribe: (listener: () => void) => {
        workspaceListeners.add(listener);
        return () => {
          workspaceListeners.delete(listener);
        };
      },
    },
    ui: {
      settings: {
        getSettingValue: () => true,
        addEventListener: () => {},
        removeEventListener: () => {},
      },
    },
    canvas: { ds: { scale: 1, offset: [0, 0] } },
    rootGraph: {
      serialize: () => copyGraph(graph),
      add: () => {},
      remove: () => {},
      onAfterChange: (_graph: unknown) => {},
    },
    async graphToPrompt() {
      return { workflow: copyGraph(graph), output: { '7': { class_type: 'SaveImage' } } };
    },
    async loadGraphData(state: object, _clean: boolean, _restore: boolean, target: HostWorkflow) {
      graph = copyGraph(state);
      if (!workflow.openWorkflows.includes(target)) workflow.openWorkflows.push(target);
      workflow.activeWorkflow = target;
      target.changeTracker ??= {
        activeState: state,
        initialState: state,
        ds: { scale: 1, offset: [0, 0] },
      };
      target.changeTracker.activeState = copyGraph(state);
      target.changeTracker.initialState = copyGraph(state);
      for (const listener of listeners) listener();
    },
  };
  /** Create a Comfy-shaped workflow whose `load` reads the saved file only when first requested. */
  function hostWorkflow(path: string, filename: string, size: number): HostWorkflow {
    return {
      path,
      filename,
      size,
      get isTemporary() {
        return this.size === -1;
      },
      get isLoaded() {
        return this.changeTracker !== null;
      },
      isLoading: false,
      isModified: false,
      content: null,
      originalContent: null,
      changeTracker: null,
      updatePath(next: string) {
        this.path = next;
      },
      async load() {
        if (this.changeTracker) return this;
        this.isLoading = true;
        await loadGate;
        if (failingLoads.has(this.path)) {
          this.isLoading = false;
          throw new Error(`Failed to load workflow: ${this.path}`);
        }
        if (this.originalContent === null) {
          const content = files.get(this.path);
          if (content === undefined) throw new Error(`Missing saved workflow file: ${this.path}`);
          this.content = content;
          this.originalContent = content;
        }
        this.isLoading = false;
        for (const listener of listeners) listener();
        // Comfy imports its change tracker after the file arrives: content is readable before the tracker exists.
        await trackerGate;
        loadedPaths.push(this.path);
        const initial = JSON.parse(this.originalContent ?? '{}') as object;
        this.changeTracker = {
          activeState: copyGraph(initial),
          initialState: initial,
          ds: { scale: 1, offset: [0, 0] },
        };
        for (const listener of listeners) listener();
        return this;
      },
      unload() {
        this.changeTracker = null;
        this.content = null;
        this.originalContent = null;
      },
    };
  }
  return {
    app,
    workflow,
    /** Register a saved workflow file in the user's workflow directory. */
    save: (path: string, state: object) => {
      files.set(path, JSON.stringify(state));
    },
    /** Open a saved workflow as Comfy's tab restoration does: listed and open, content unread. */
    openInBackground: (path: string): HostWorkflow => {
      const created = workflow.createSaved(path);
      workflow.openWorkflows.push(created);
      for (const listener of listeners) listener();
      return created;
    },
    /** Open a saved workflow as the workflow store does: the tab opens before its content is read. */
    open: async (path: string): Promise<HostWorkflow> => {
      const created =
        workflow.openWorkflows.find((item) => item.path === path) ?? workflow.createSaved(path);
      if (!workflow.openWorkflows.includes(created)) workflow.openWorkflows.push(created);
      for (const listener of listeners) listener();
      const loaded = await created.load();
      await app.loadGraphData(loaded.changeTracker?.activeState ?? {}, true, true, loaded);
      return loaded;
    },
    /**
     * Reopen tabs as Comfy restores them from its own persistence at startup, in place of its
     * default tab: each tab carries its own document, and only the active one is loaded.
     */
    reopen: async (
      documents: readonly {
        readonly name: string;
        readonly path: string | null;
        readonly temporary: boolean;
        readonly graphJson: string;
        readonly baselineGraphJson: string | null;
      }[],
      active: number,
    ): Promise<HostWorkflow[]> => {
      workflow.openWorkflows = [];
      workflow.activeWorkflow = null;
      const reopened = documents.map((value) => {
        const created = value.temporary
          ? hostWorkflow(`workflows/${value.name}.json`, value.name, -1)
          : hostWorkflow(
              value.path ?? `workflows/${value.name}.json`,
              value.name,
              value.graphJson.length,
            );
        created.content = value.graphJson;
        created.originalContent = value.baselineGraphJson ?? value.graphJson;
        workflow.openWorkflows.push(created);
        return created;
      });
      for (const listener of listeners) listener();
      const shown = reopened[active];
      if (!shown) throw new Error('Reopened active tab is unavailable.');
      await app.loadGraphData(JSON.parse(shown.content ?? '{}') as object, true, true, shown);
      return reopened;
    },
    /** Hold every saved-workflow tracker construction, after its content arrived, until released. */
    holdTrackers: (): (() => void) => {
      let release: () => void = () => {};
      trackerGate = new Promise<void>((resolve) => {
        release = resolve;
      });
      return release;
    },
    /** Hold every saved-workflow read until the returned release is called. */
    holdLoads: (): (() => void) => {
      let release: () => void = () => {};
      loadGate = new Promise<void>((resolve) => {
        release = resolve;
      });
      return release;
    },
    failLoad: (path: string) => {
      failingLoads.add(path);
    },
    loaded: () => [...loadedPaths],
    committed: () => {
      app.extensionManager.spinner = false;
      for (const listener of workspaceListeners) listener();
    },
    setGraph: (value: object) => {
      graph = copyGraph(value);
    },
    notify: () => {
      for (const listener of listeners) listener();
    },
  };
}

/** Copy only JSON workflow fixtures, matching the serialized external boundary. */
function copyGraph(value: object): object {
  return JSON.parse(JSON.stringify(value)) as object;
}
