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
  isModified: boolean;
  content: string;
  originalContent: string;
  changeTracker: {
    activeState: object;
    initialState: object;
    ds: { scale: number; offset: number[] };
  } | null;
  updatePath(path: string): void;
}

/** Model the exposed workflow store and actual load reset/baseline behavior at the Comfy boundary. */
export function comfyDesktopHostBoundary() {
  let graph: object = { nodes: [], extra: { title: 'active' } };
  let key = 0;
  const workspaceListeners = new Set<() => void>();
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
  return {
    app,
    workflow,
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
