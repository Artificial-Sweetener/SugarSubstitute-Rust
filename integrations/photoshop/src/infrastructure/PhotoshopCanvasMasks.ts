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
/** Own Photoshop mask identities, committed selection editing and coherent full-resolution captures. */
import { app } from 'photoshop';
import { runPhotoshopModal } from './PhotoshopModalExecution.js';
import type {
  CanvasMaskQuery,
  CanvasMaskReference,
  CanvasMaskResult,
  CanvasMaskSnapshot,
} from '../../../contracts/CanvasMasksProtocol.generated.js';
import {
  canvasDocumentIdentity,
  findCanvasDocument,
  type PhotoshopDocument,
} from './PhotoshopCanvasIdentity.js';
import { putSelectionPixels } from './PhotoshopMaskPixels.js';
import { readSelectionPixels } from './PhotoshopSelectionPixels.js';
import { PhotoshopCommittedChanges } from './PhotoshopCommittedChanges.js';
import type { PhotoshopMaskSessionPort } from './PhotoshopMaskSession.js';
type Channel = PhotoshopDocument['channels'][number];
type AlphaChannel = Parameters<PhotoshopDocument['selection']['saveTo']>[0];

/** Preserve each region in a dedicated alpha channel and expose selection editing only for the selected entry. */
export class PhotoshopCanvasMasks {
  #active: CanvasMaskReference | null = null;
  #busy = false;
  #disposed = false;
  #tail: Promise<unknown> = Promise.resolve();
  readonly #changes: PhotoshopCommittedChanges;
  /** Publish only completed native edits through the shared mask transport. */
  constructor(
    private readonly changed: (documentId: string, maskId: string, revision: string) => void,
    report: (error: unknown) => void,
    private readonly session: PhotoshopMaskSessionPort,
  ) {
    const retained = this.session.read();
    if (retained) {
      const document = findCanvasDocument(retained.mask.document.id);
      if (
        document &&
        canvasDocumentIdentity(document).geometryRevision ===
          retained.mask.document.geometryRevision &&
        String(document.activeHistoryState.id) === retained.revision &&
        document.channels.some((channel) => channel.name === retained.mask.id && isAlpha(channel))
      )
        this.#active = retained.mask;
    }
    this.#changes = new PhotoshopCommittedChanges(
      () => {
        if (!this.#active) return null;
        const document = app.activeDocument;
        return document &&
          String(document.id) === this.#active.document.id &&
          !document.quickMaskMode
          ? JSON.stringify([document.id, document.activeHistoryState.id])
          : null;
      },
      () => this.#busy,
      async () => {
        await this.#tail;
        const active = this.#active;
        if (
          this.#disposed ||
          !active ||
          String(app.activeDocument?.id) !== active.document.id ||
          app.activeDocument?.quickMaskMode
        )
          return;
        this.#document(active);
        this.#remember();
        this.changed(active.document.id, active.id, this.#revision());
      },
      report,
    );
    if (this.#active) this.#changes.start();
  }
  /** Keep all selection mutations and captures in one native operation sequence. */
  execute(command: CanvasMaskQuery): Promise<CanvasMaskResult> {
    return this.#queue(() => this.#execute(command));
  }
  /** Release native observation without deleting authored channels from a user's document. */
  dispose(): void {
    this.#remember();
    this.#disposed = true;
    this.#active = null;
    this.#changes.dispose();
  }
  /** Refuse queued work after teardown and keep one failure from poisoning later requests. */
  #queue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.#tail.then(() => {
      if (this.#disposed) throw new Error('Photoshop mask adapter closed.');
      return operation();
    });
    this.#tail = result.catch(() => undefined);
    return result;
  }
  /** Dispatch native capabilities while requiring exact document geometry for every bound mask. */
  async #execute(command: CanvasMaskQuery): Promise<CanvasMaskResult> {
    if (command.type === 'context')
      return {
        kind: 'context',
        supported: true,
        observesChanges: true,
        document: app.activeDocument ? canvasDocumentIdentity(app.activeDocument) : null,
      };
    if (command.type === 'release') {
      if (
        !command.mask ||
        (this.#active?.id === command.mask.id &&
          this.#active.document.id === command.mask.document.id)
      ) {
        const active = this.#active;
        try {
          if (
            active &&
            String(app.activeDocument?.id) === active.document.id &&
            !app.activeDocument?.quickMaskMode
          )
            await this.#modal(
              'Save canvas mask',
              async () => {
                await this.#save(this.#document(active), active);
                return { kind: 'activated' };
              },
              true,
            );
        } finally {
          this.#active = null;
          this.session.write(null);
          this.#changes.stop();
        }
      }
      return { kind: 'activated' };
    }
    const document =
      command.type === 'create'
        ? findCanvasDocument(command.document.id)
        : command.type === 'activate'
          ? this.#document(command.mask)
          : command.masks[0]
            ? this.#document(command.masks[0])
            : null;
    if (!document)
      return {
        kind: 'failure',
        status: 'unavailable',
        message: 'Open the Photoshop document used by this mask.',
      };
    if (app.activeDocument?.id !== document.id)
      return {
        kind: 'failure',
        status: 'stale',
        message: 'Activate the Photoshop document bound to this mask.',
      };
    if (document.quickMaskMode || document.artboards.length)
      return {
        kind: 'failure',
        status: 'unsupported',
        message: 'Finish Quick Mask editing or use a document without artboards.',
      };
    return this.#modal(
      command.type === 'capture' ? 'Capture canvas masks' : 'Edit canvas mask',
      async () => {
        if (app.activeDocument?.id !== document.id)
          throw new Error('The active Photoshop document changed before mask editing.');
        if (command.type === 'create') {
          if (
            canvasDocumentIdentity(document).geometryRevision !== command.document.geometryRevision
          )
            throw new Error('The Photoshop document geometry changed.');
          if (this.#active && this.#active.document.id === String(document.id))
            await this.#save(document, this.#active);
          const name = `SugarCubes ${command.name.slice(0, 64)} ${crypto.randomUUID()}`;
          const channel = document.channels.add();
          channel.name = name;
          document.activeChannels = [...document.componentChannels];
          try {
            if (command.source === 'pixels') {
              if (!command.pixels) throw new Error('Mask pixels are missing.');
              await putSelectionPixels(document, command.pixels);
            } else if (command.source === 'blank') await document.selection.deselect();
            if (document.selection.bounds) await document.selection.saveTo(this.#alpha(channel));
            const mask: CanvasMaskReference = {
              canvasId: 'photoshop',
              document: canvasDocumentIdentity(document),
              id: name,
            };
            this.#active = mask;
            return { kind: 'created', mask };
          } catch (error: unknown) {
            await channel.remove();
            throw error;
          }
        }
        if (command.type === 'activate') {
          if (this.#active && this.#active.document.id === String(document.id))
            await this.#save(this.#document(this.#active), this.#active);
          await document.selection.load(this.#channel(document, command.mask));
          document.activeChannels = [...document.componentChannels];
          this.#active = command.mask;
          return { kind: 'activated' };
        }
        if (this.#active && this.#active.document.id === String(document.id))
          await this.#save(this.#document(this.#active), this.#active);
        const snapshots: CanvasMaskSnapshot[] = [];
        const revision = this.#revision();
        let transferred = 0;
        for (const mask of command.masks) {
          const source = this.#document(mask);
          if (source !== document) throw new Error('A mask capture must use one document.');
          await source.selection.load(this.#channel(source, mask));
          const coverage = await readSelectionPixels(source, command.maximumSize);
          const pixels = { width: coverage.width, height: coverage.height, data: coverage.data };
          transferred += pixels.width * pixels.height;
          if (transferred > 64 * 1024 * 1024)
            throw new Error('The mask collection exceeds the supported capture size.');
          snapshots.push({
            mask,
            revision,
            pixels,
          });
        }
        return { kind: 'captured', snapshots };
      },
      command.type !== 'capture',
    );
  }
  /** Roll back capture-only selection changes and partial mutations in one native history scope. */
  async #modal(
    name: string,
    operation: () => Promise<CanvasMaskResult>,
    commit: boolean,
  ): Promise<CanvasMaskResult> {
    let result: CanvasMaskResult = {
      kind: 'failure',
      status: 'error',
      message: 'Photoshop did not execute the mask operation.',
    };
    this.#busy = true;
    try {
      await runPhotoshopModal(name, async (execution) => {
        const document = app.activeDocument;
        if (!document) throw new Error('No Photoshop document is active.');
        const history = await execution.hostControl.suspendHistory({
          documentID: document.id,
          name,
        });
        let succeeded = false;
        try {
          result = await operation();
          succeeded = true;
        } finally {
          await execution.hostControl.resumeHistory(history, succeeded && commit);
        }
      });
      return result;
    } finally {
      this.#busy = false;
      if (commit) {
        if (this.#active) this.#changes.start();
        this.#changes.acceptCurrent();
        this.#remember();
      }
    }
  }
  /** Validate identity before reading a channel or modifying its selection. */
  #document(mask: CanvasMaskReference): PhotoshopDocument {
    const document = findCanvasDocument(mask.document.id);
    if (
      mask.canvasId !== 'photoshop' ||
      !document ||
      canvasDocumentIdentity(document).geometryRevision !== mask.document.geometryRevision ||
      document.width !== mask.document.width ||
      document.height !== mask.document.height ||
      app.activeDocument?.id !== document.id
    )
      throw new Error('The mask document is closed, inactive, or its geometry changed.');
    return document;
  }
  /** Resolve opaque channel names exactly; renamed or deleted channels never bind another region. */
  #channel(document: PhotoshopDocument, mask: CanvasMaskReference): AlphaChannel {
    const channel = document.channels.getByName(mask.id);
    if (!channel || !isAlpha(channel))
      throw new Error('The bound Photoshop mask channel is missing.');
    return channel;
  }
  /** Narrow the native AlphaChannel subtype required by Photoshop's selection API. */
  #alpha(channel: Channel): AlphaChannel {
    if (!isAlpha(channel)) throw new Error('Photoshop did not create an alpha channel.');
    return channel;
  }
  /** Persist empty selections as empty channels rather than Photoshop's implicit select-all behavior. */
  async #save(document: PhotoshopDocument, mask: CanvasMaskReference): Promise<void> {
    const channel = this.#channel(document, mask);
    if (document.activeChannels.some((current) => current.name === mask.id)) return;
    if (document.selection.bounds) await document.selection.saveTo(this.#alpha(channel));
    else {
      const names = document.activeChannels.map((current) => current.name);
      await channel.remove();
      const blank = document.channels.add();
      blank.name = mask.id;
      document.activeChannels = document.channels.filter((current) => names.includes(current.name));
    }
  }
  /** Label captures by the adapter's current committed document state. */
  #revision(): string {
    return String(app.activeDocument?.activeHistoryState.id ?? 'unavailable');
  }

  /** Retain identity only while the current native history still owns the unsaved selection coverage. */
  #remember(): void {
    const active = this.#active;
    const document = active ? findCanvasDocument(active.document.id) : null;
    this.session.write(
      active && document
        ? { mask: active, revision: String(document.activeHistoryState.id) }
        : null,
    );
  }
}

/** Validate the native subtype before crossing its declared API boundary. */
function isAlpha(channel: Channel): channel is AlphaChannel {
  return 'typename' in channel && channel.typename === 'AlphaChannel';
}
