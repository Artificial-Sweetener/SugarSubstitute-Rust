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
/** Clipboard requests carry transient source text only across the explicitly owned native transfer boundary. */

/** Exact internal transfers bypass ingestion; external plain text follows the current editor mode. */
export interface ClipboardTransfer {
  readonly kind: 'exact' | 'plain';
  readonly text: string;
}

/** Browser availability and permission rejection are expected outcomes rather than empty successful reads. */
export type ClipboardReadResult =
  | { readonly status: 'read'; readonly value: ClipboardTransfer }
  | { readonly status: 'empty' | 'denied' | 'unavailable' };

/** A completed native write is distinct from an unavailable or denied system boundary. */
export type ClipboardWriteResult =
  | { readonly status: 'written' }
  | { readonly status: 'denied' | 'unavailable' };

/** External implementations return unknown data for validation; cancellation prevents owned publication even if native work cannot abort. */
export interface ClipboardGateway {
  readonly readable: boolean;
  readonly writable: boolean;
  /** A trusted native paste may carry its already captured transfer; adapters retain it when programmatic permission is unavailable. */
  read(signal: AbortSignal, nativeTransfer?: ClipboardTransfer): Promise<unknown>;
  /** Write the deliberately captured selection; native acceptance may outlive cancelled acknowledgement. */
  write(text: string, signal: AbortSignal): Promise<unknown>;
}

/** Only the currently owned request may publish a validated clipboard value. */
export type ClipboardReadOutcome =
  | ClipboardReadResult
  | { readonly status: 'cancelled' | 'disposed' };

/** Native writes may complete after retirement; retired requests never authorize a subsequent source cut. */
export type ClipboardWriteOutcome =
  | ClipboardWriteResult
  | { readonly status: 'cancelled' | 'disposed' };
