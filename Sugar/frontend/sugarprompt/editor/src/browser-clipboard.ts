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
/** Native Clipboard API conversion owns system MIME spelling and expected browser permission failures. */
import { requireValue } from '../../core/src/required.js';
import type {
  ClipboardGateway,
  ClipboardReadResult,
  ClipboardTransfer,
  ClipboardWriteResult,
} from '../../services/src/clipboard/contracts.js';
import { encodeSource } from '../../core/src/line-endings.js';
import { exactClipboardMime } from './clipboard.js';
import { clipboardWriter } from './browser-clipboard-write.js';
/** Translate only recognized platform availability errors; unexpected causes remain observable. */
function unavailable(owner: Window & typeof globalThis, cause: unknown): 'denied' | 'unavailable' {
  if (cause instanceof owner.DOMException) {
    if (cause.name === 'NotAllowedError' || cause.name === 'SecurityError') return 'denied';
    if (cause.name === 'NotSupportedError') return 'unavailable';
    // Chromium rejects reading an item after the system clipboard changed beneath it.
    if (cause.name === 'InvalidStateError') return 'unavailable';
  }
  throw cause;
}
/** One mounted editor captures its document's native clipboard, including the exact private format used by keyboard events. */
export function browserClipboard(owner: Window & typeof globalThis): ClipboardGateway {
  const clipboard = owner.navigator.clipboard;
  const privateType = `web ${exactClipboardMime}`;
  const readable = !!clipboard && typeof clipboard.read === 'function';
  const writable =
    !!clipboard &&
    typeof clipboard.write === 'function' &&
    typeof owner.ClipboardItem === 'function' &&
    typeof owner.ClipboardItem.supports === 'function' &&
    owner.ClipboardItem.supports(privateType);
  /** Already accepted system writes finish in intent order even when publication is retired; failures remain owned by their original caller. */
  async function write(text: string): Promise<ClipboardWriteResult> {
    try {
      await clipboard.write([
        new owner.ClipboardItem({
          'text/plain': new owner.Blob([text], { type: 'text/plain' }),
          [privateType]: new owner.Blob([text], { type: exactClipboardMime }),
        }),
      ]);
      return { status: 'written' };
    } catch (cause) {
      return { status: unavailable(owner, cause) };
    }
  }
  const writer = clipboard ? clipboardWriter(clipboard, write) : undefined;
  /** Trusted event text remains the paste snapshot when programmatic reading cannot recover a corresponding exact format. */
  function nativeResult(transfer: ClipboardTransfer): ClipboardReadResult {
    return transfer.text ? { status: 'read', value: transfer } : { status: 'empty' };
  }
  return {
    readable,
    writable,
    /** Prefer exact internal source across all items; HTML and other formats never become text implicitly. */
    async read(signal, nativeTransfer) {
      if (!readable || signal.aborted) return { status: 'unavailable' };
      try {
        const pending = writer?.settled();
        if (pending) await pending;
        if (signal.aborted) return { status: 'unavailable' };
        const items = await clipboard.read();
        for (const type of [privateType, exactClipboardMime, 'text/plain'])
          for (const item of items)
            if (item.types.includes(type)) {
              const text = await (await item.getType(type)).text();
              if (nativeTransfer) {
                if (type === 'text/plain' || !item.types.includes('text/plain'))
                  return nativeResult(nativeTransfer);
                const plain = await (await item.getType('text/plain')).text();
                if (encodeSource(plain).logical !== encodeSource(nativeTransfer.text).logical)
                  return nativeResult(nativeTransfer);
              }
              return {
                status: 'read',
                value: {
                  kind: type === 'text/plain' ? 'plain' : 'exact',
                  text,
                },
              };
            }
        return nativeTransfer ? nativeResult(nativeTransfer) : { status: 'empty' };
      } catch (cause) {
        const status = unavailable(owner, cause);
        return nativeTransfer ? nativeResult(nativeTransfer) : { status };
      }
    },
    /** Literal plain text and private exact source share one native atomic clipboard item. */
    write(text, signal) {
      if (!writable || signal.aborted) return Promise.resolve({ status: 'unavailable' });
      return requireValue(writer, 'browser-clipboard').submit(text);
    },
  };
}
