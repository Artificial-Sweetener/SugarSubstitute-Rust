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
/** Model the installed host's reused graph, configuration hooks and later workflow commitment. */

export interface WorkflowFixture {
  path: string;
  filename: string;
  serializedId: string;
  nodes: Array<{ id: string; title: string }>;
  changeTracker: { activeState: object; nodeOutputs: Record<string, unknown> };
}

/** Construct distinct host objects with deliberately colliding node and serialized workflow IDs. */
export function workflowFixture(name: string): WorkflowFixture {
  return {
    path: `workflows/${name}.json`,
    filename: name,
    serializedId: 'duplicate-workflow-id',
    nodes: [
      { id: '7', title: 'Image' },
      { id: '42:23', title: 'Detail' },
    ],
    changeTracker: { activeState: {}, nodeOutputs: {} },
  };
}

/** Expose the public host shape while allowing tests to pause after configuration hooks. */
export class WorkflowHostFixture {
  readonly #subscribers = new Set<() => void>();
  readonly graph = { nodes: [] as WorkflowFixture['nodes'] };
  readonly extensionManager = {
    workflow: {
      activeWorkflow: null as WorkflowFixture | null,
      openWorkflows: [] as WorkflowFixture[],
      $subscribe:
        (_listener: () => void, _options: { detached: boolean; flush: string }): (() => void) =>
        () =>
          undefined,
    },
  };
  beforeConfigure: () => void = () => undefined;
  afterConfigure: () => void = () => undefined;
  pauseAfterConfigure: (() => Promise<void>) | null = null;

  /** Bind an explicit subscriber lifecycle resembling the public Pinia store contract. */
  constructor(initial: WorkflowFixture) {
    this.extensionManager.workflow.activeWorkflow = initial;
    this.extensionManager.workflow.openWorkflows = [initial];
    this.graph.nodes = [...initial.nodes];
    this.extensionManager.workflow.$subscribe = (listener) => {
      this.#subscribers.add(listener);
      return () => this.#subscribers.delete(listener);
    };
  }

  /** Count listeners for repeated installation and teardown proofs. */
  get subscriberCount(): number {
    return this.#subscribers.size;
  }

  /** Model hooks before active workflow commitment and preserve the same root graph object. */
  async loadGraphData(
    _data: unknown,
    _clean = true,
    _restore = true,
    workflow?: WorkflowFixture,
  ): Promise<void> {
    const target = workflow ?? this.extensionManager.workflow.activeWorkflow;
    if (!target) throw new Error('Fixture workflow is unavailable.');
    this.beforeConfigure();
    this.graph.nodes = [...target.nodes];
    this.afterConfigure();
    if (this.pauseAfterConfigure) await this.pauseAfterConfigure();
    target.changeTracker.activeState = {};
    this.extensionManager.workflow.activeWorkflow = target;
    if (!this.extensionManager.workflow.openWorkflows.includes(target))
      this.extensionManager.workflow.openWorkflows.push(target);
    this.notify();
  }

  /** Notify only explicit store changes, leaving DOM/popup activity outside this fixture. */
  notify(): void {
    for (const listener of this.#subscribers) listener();
  }

  /** Close an open binding while retaining the host object to test explicit reopening. */
  close(workflow: WorkflowFixture): void {
    this.extensionManager.workflow.openWorkflows =
      this.extensionManager.workflow.openWorkflows.filter((item) => item !== workflow);
    if (this.extensionManager.workflow.activeWorkflow === workflow)
      this.extensionManager.workflow.activeWorkflow = null;
    this.notify();
  }
}
