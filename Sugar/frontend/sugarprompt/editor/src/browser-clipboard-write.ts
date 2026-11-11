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
/** One browser clipboard lifetime serializes irreversible system writes with at most one current pending payload. */
import type { ClipboardWriteResult } from '../../services/src/clipboard/contracts.js';

/** An accepted write owns only its operation payload and caller settlement; it never owns editor state. */
interface PendingWrite {
  readonly text: string;
  readonly complete: (result: ClipboardWriteResult) => void;
  readonly fail: (cause: unknown) => void;
}

/** Native system writes cannot abort; superseded pending writes retire before I/O while the newest intent follows the active call. */
export class BrowserClipboardWriter {
  private readonly write: (text: string) => Promise<ClipboardWriteResult>;
  private pending: PendingWrite | undefined;
  private idle: Promise<void> | undefined;
  private settleIdle: (() => void) | undefined;

  /** The browser adapter supplies one literal native write implementation and preserves its original error causes. */
  constructor(write: (text: string) => Promise<ClipboardWriteResult>) {
    this.write = write;
  }

  /** Reads await actual write settlement even when the editor caller retired its acknowledgement. */
  settled(): Promise<void> | undefined {
    return this.idle;
  }

  /** Keep one active call and one newest pending payload; old pending callers settle without another system write. */
  submit(text: string): Promise<ClipboardWriteResult> {
    return new Promise((complete, fail) => {
      const request = { text, complete, fail };
      if (this.idle) {
        this.pending?.complete({ status: 'unavailable' });
        this.pending = request;
      } else {
        this.idle = new Promise((resolve) => {
          this.settleIdle = resolve;
        });
        this.start(request);
      }
    });
  }

  /** Both success and failure release the physical lane; failure remains observable to the original write caller. */
  private start(request: PendingWrite): void {
    let result: Promise<ClipboardWriteResult>;
    try {
      result = this.write(request.text);
    } catch (cause) {
      request.fail(cause);
      this.advance();
      return;
    }
    void result.then(request.complete, request.fail).then(() => this.advance());
  }

  /** Release completed operation data before admitting the newest accepted system intent. */
  private advance(): void {
    const next = this.pending;
    this.pending = undefined;
    if (next) this.start(next);
    else {
      const settle = this.settleIdle;
      this.idle = undefined;
      this.settleIdle = undefined;
      settle?.();
    }
  }
}

const writers = new WeakMap<Clipboard, BrowserClipboardWriter>();

/** Editors and baseline replacements in one window share the actual system boundary; weak keys and settled requests retain no native view. */
export function clipboardWriter(
  clipboard: Clipboard,
  write: (text: string) => Promise<ClipboardWriteResult>,
): BrowserClipboardWriter {
  let writer = writers.get(clipboard);
  if (!writer) {
    writer = new BrowserClipboardWriter(write);
    writers.set(clipboard, writer);
  }
  return writer;
}
