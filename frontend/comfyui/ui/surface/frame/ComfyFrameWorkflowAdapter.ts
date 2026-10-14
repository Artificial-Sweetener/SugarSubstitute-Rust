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
/** Adapt Comfy's mounted workflow action surface without reimplementing workflow policies. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { WorkflowSessionContext } from '../../execution/domain/WorkflowSession.js';
import type { WorkflowSessionId } from '../../execution/domain/ExecutionIdentity.js';

export interface FrameWorkflowTab {
  readonly id: WorkflowSessionId;
  readonly title: string;
  readonly modified: boolean;
  readonly selected: boolean;
}

/** Keep external component compatibility isolated from the Fluent view and session identity owner. */
export class ComfyFrameWorkflowAdapter {
  readonly #manager: UnknownRecord;
  readonly #workflow: UnknownRecord;

  /** Validate the public store and retain the existing native tab component as its action port. */
  constructor(
    app: unknown,
    private readonly native: HTMLElement,
  ) {
    const manager = isRecord(app) && isRecord(app.extensionManager) ? app.extensionManager : null;
    if (!manager || !isRecord(manager.workflow))
      throw new Error('Comfy workflow controls are unavailable.');
    this.#manager = manager;
    this.#workflow = manager.workflow;
  }

  /** Project only host-owned labels, order and dirty state onto existing session identities. */
  read(sessions: readonly WorkflowSessionContext[]): readonly FrameWorkflowTab[] {
    return this.#open().flatMap((workflow) => {
      const session = sessions.find((candidate) => candidate.path === workflow.path);
      return session
        ? [
            {
              id: session.sessionId,
              title: session.title,
              modified: workflow.isModified === true,
              selected: workflow === this.#workflow.activeWorkflow,
            },
          ]
        : [];
    });
  }

  /** Delegate activation to the same native handler Comfy uses for its visible tab. */
  activate(id: WorkflowSessionId, sessions: readonly WorkflowSessionContext[]): void {
    const tab = this.#nativeTab(id, sessions);
    const control = tab.closest<HTMLElement>('button');
    if (!control) throw new Error('Comfy workflow activation control is unavailable.');
    control.click();
  }

  /** Delegate middle-click close to Comfy's confirmation and replacement-workflow service. */
  close(id: WorkflowSessionId, sessions: readonly WorkflowSessionContext[]): void {
    const tab = this.#nativeTab(id, sessions);
    const MouseEventType = this.native.ownerDocument.defaultView?.MouseEvent;
    if (!MouseEventType) throw new Error('Comfy workflow close event is unavailable.');
    // Vue compiles WorkflowTab @click.middle to mouseup; click opens its hover preview instead.
    tab.dispatchEvent(
      new MouseEventType('mouseup', { button: 1, bubbles: true, cancelable: true }),
    );
  }

  /** Reorder through the host store without mutating workflow contents or session identities. */
  reorder(from: number, to: number): void {
    const action = this.#workflow.reorderWorkflows;
    if (typeof action !== 'function') throw new Error('Comfy workflow ordering is unavailable.');
    action.call(this.#workflow, from, to);
  }

  /** Dispatch an existing Comfy command while retaining its native policy and error handling. */
  async command(id: string): Promise<void> {
    const commands = this.#manager.command;
    if (!isRecord(commands) || typeof commands.execute !== 'function')
      throw new Error('Comfy commands are unavailable.');
    await commands.execute.call(commands, id);
  }

  /** Subscribe only to the workflow store, independently of graph discovery and popup mounts. */
  subscribe(listener: () => void): () => void {
    const action = this.#workflow.$subscribe;
    if (typeof action !== 'function') throw new Error('Comfy workflow observation is unavailable.');
    const stop: unknown = action.call(this.#workflow, listener, { detached: true, flush: 'post' });
    if (typeof stop !== 'function') throw new Error('Comfy workflow teardown is unavailable.');
    return () => {
      stop();
    };
  }

  /** Fail closed if the native action surface no longer matches the public ordered workflows. */
  #nativeTab(id: WorkflowSessionId, sessions: readonly WorkflowSessionContext[]): HTMLElement {
    const session = sessions.find((candidate) => candidate.sessionId === id);
    const open = this.#open();
    const index = open.findIndex((workflow) => workflow.path === session?.path);
    const tabs = this.native.querySelectorAll<HTMLElement>('.workflow-tab');
    const tab = tabs.item(index);
    const label = tab?.querySelector('.workflow-label')?.textContent?.trim();
    if (!session || index < 0 || tabs.length !== open.length || !tab || label !== session.title)
      throw new Error('Comfy workflow action surface changed; retry after tabs finish updating.');
    return tab;
  }

  /** Narrow the dynamic host collection before exposing any workflow facts. */
  #open(): readonly UnknownRecord[] {
    const workflows = this.#workflow.openWorkflows;
    if (!Array.isArray(workflows) || !workflows.every(isRecord))
      throw new Error('Comfy open workflows are malformed.');
    return workflows;
  }
}
