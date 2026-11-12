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
/** One finite delegated lifetime owns external model-page and metadata work, independently of native menu placement. */
import type { EditorView, ViewUpdate } from '@codemirror/view';
import { LoraPageSession, LoraMetadataSession } from '../../services/src/index.js';
import { loraPageHost, loraMetadataHost } from './lora-action-hosts.js';
import { editorFreshness } from './freshness.js';
import { loraPresentation } from './lora-presentation.js';
import { currentLoraTarget, type PreparedLoraMenu } from './lora-menu-targets.js';
import type { ContextMenuItem } from './context-menu-contracts.js';

/** Native context retirement settles service work; only source-current outcomes can reach the host. */
export class LoraMenuActions {
  private readonly view: EditorView;
  private session: LoraPageSession;
  private metadataSession: LoraMetadataSession;
  private operation = 0;
  private disposed = false;

  /** Capture deliberate external capabilities for this mounted editor. */
  constructor(view: EditorView) {
    this.view = view;
    this.session = new LoraPageSession(view.state.facet(loraPageHost)?.adapter);
    this.metadataSession = new LoraMetadataSession(view.state.facet(loraMetadataHost)?.capability);
  }

  /** Capability replacement disposes its old service owner; other context retirement cancels current work. */
  update(update: ViewUpdate): void {
    if (
      update.startState.facet(loraPageHost) !== update.state.facet(loraPageHost) ||
      update.startState.facet(loraMetadataHost) !== update.state.facet(loraMetadataHost)
    ) {
      this.session.dispose();
      this.metadataSession.dispose();
      this.session = new LoraPageSession(update.state.facet(loraPageHost)?.adapter);
      this.metadataSession = new LoraMetadataSession(
        update.state.facet(loraMetadataHost)?.capability,
      );
    } else if (
      update.startState.field(editorFreshness) !== update.state.field(editorFreshness) ||
      update.startState.field(loraPresentation) !== update.state.field(loraPresentation) ||
      this.view.composing ||
      this.view.compositionStarted
    ) {
      this.session.cancel();
      this.metadataSession.cancel();
    }
  }

  /** Accept a source-current leaf; external adoption must retain this operation's lifetime before acknowledgement. */
  async execute(menu: PreparedLoraMenu, item: ContextMenuItem): Promise<void> {
    if (this.disposed || menu.kind !== 'inline' || !currentLoraTarget(this.view.state, menu))
      return;
    const operation = ++this.operation;
    this.session.cancel();
    this.metadataSession.cancel();
    let adopted = false;
    try {
      if (item.kind === 'page' && menu.pageHost) {
        const outcome = await this.session.open(item.request);
        if (!this.disposed && operation === this.operation)
          menu.pageHost.observe(
            currentLoraTarget(this.view.state, menu) ? outcome : { status: 'cancelled' },
          );
      } else if (item.kind === 'action' && menu.metadataHost) {
        const outcome = await this.metadataSession.execute(item.request);
        if (this.disposed || operation !== this.operation) return;
        if (!currentLoraTarget(this.view.state, menu))
          menu.metadataHost.observe({ status: 'cancelled' });
        else if (outcome.status === 'updated') {
          adopted = true;
          menu.metadataHost.accept(outcome.adapter);
          if (this.disposed || operation !== this.operation) return;
          menu.metadataHost.observe({
            status: 'updated',
            catalogRevision: outcome.adapter.revision,
          });
        } else menu.metadataHost.observe(outcome);
      }
    } catch (cause) {
      if (
        !this.disposed &&
        operation === this.operation &&
        (adopted || currentLoraTarget(this.view.state, menu))
      )
        (item.kind === 'page' ? menu.pageHost : menu.metadataHost)?.report(cause);
    }
  }

  /** No retained service request can publish after native teardown. */
  dispose(): void {
    this.disposed = true;
    this.operation++;
    this.session.dispose();
    this.metadataSession.dispose();
  }
}
