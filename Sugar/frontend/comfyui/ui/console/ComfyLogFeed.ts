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
/**
 * Read ComfyUI's own output through the log feed its Logs panel uses.
 *
 * ComfyUI keeps its recent output (`getRawLogs`), and streams new output as `logs` events to a
 * client that subscribed (`subscribeLogs`). Only one subscription is held at a time, and it is
 * released when the console closes.
 */
import { isRecord } from '../types/common.js';

/** The part of ComfyUI's API client the feed uses, validated before use. */
interface LogApi {
  getRawLogs(): Promise<unknown>;
  subscribeLogs(enabled: boolean): Promise<unknown>;
  addEventListener(type: string, listener: (event: Event) => void): void;
  removeEventListener(type: string, listener: (event: Event) => void): void;
}

/** Narrow ComfyUI's API client to the log calls, or null when it lacks them. */
function readLogApi(value: unknown): LogApi | null {
  if (!isRecord(value)) return null;
  const calls = ['getRawLogs', 'subscribeLogs', 'addEventListener', 'removeEventListener'];
  return calls.every((name) => typeof value[name] === 'function')
    ? (value as unknown as LogApi)
    : null;
}

/** The text lines of a log payload: `{ entries: [{ m }] }`, each message possibly several lines. */
export function readLogLines(value: unknown): readonly string[] {
  const entries = isRecord(value) ? value.entries : null;
  if (!Array.isArray(entries)) return [];
  const lines: string[] = [];
  for (const entry of entries) {
    const message = isRecord(entry) ? entry.m : null;
    if (typeof message !== 'string') continue;
    const parts = message.replace(/\r\n?/g, '\n').split('\n');
    if (parts.at(-1) === '') parts.pop();
    lines.push(...parts);
  }
  return lines;
}

/** One open subscription to ComfyUI's output. */
export class ComfyLogFeed {
  readonly #api: LogApi;
  #listener: ((event: Event) => void) | null = null;

  /** Bind to ComfyUI's API client; throws when it offers no log feed. */
  constructor(api: unknown) {
    const logs = readLogApi(api);
    if (!logs) throw new Error('ComfyUI offers no log feed.');
    this.#api = logs;
  }

  /** Replay the kept output, then deliver every new line to `receive` until `close`. */
  async open(receive: (lines: readonly string[]) => void): Promise<readonly string[]> {
    this.#close();
    const listener = (event: Event): void => {
      const detail = event instanceof CustomEvent ? (event.detail as unknown) : null;
      const lines = readLogLines(detail);
      if (lines.length) receive(lines);
    };
    this.#listener = listener;
    this.#api.addEventListener('logs', listener);
    await this.#api.subscribeLogs(true);
    return readLogLines(await this.#api.getRawLogs());
  }

  /** Stop receiving and end the subscription. */
  async close(): Promise<void> {
    if (!this.#close()) return;
    await this.#api.subscribeLogs(false);
  }

  /** Remove the listener; whether one was attached. */
  #close(): boolean {
    if (!this.#listener) return false;
    this.#api.removeEventListener('logs', this.#listener);
    this.#listener = null;
    return true;
  }
}
