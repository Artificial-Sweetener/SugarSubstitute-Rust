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
/** Retain GIMP browser contexts across interrupted setup, interaction and teardown. */
import type { Logger } from '@playwright/test';

/** Expose the context operations whose ownership survives an interrupted page acquisition. */
export interface GimpFixtureContext<Page> {
  newPage(): Promise<Page>;
  close(): Promise<void>;
}

/** Acquire a private context with bounded operation diagnostics. */
export interface GimpFixtureBrowser<Page> {
  newContext(options: { logger: Logger }): Promise<GimpFixtureContext<Page>>;
}

/** Describe the last browser activity without retaining arguments or an unbounded event history. */
export interface GimpFixtureDiagnostics {
  phase: string;
  pending: readonly string[];
  recent: readonly string[];
}

/** Own one test's context before page setup and keep late acquisitions subject to retirement. */
export class GimpBrowserPageLifetime<Page> {
  #context: Promise<GimpFixtureContext<Page>> | undefined;
  #page: Page | undefined;
  #retirement: Promise<void> | undefined;
  #retired = false;
  #phase = 'new';
  readonly #pending: string[] = [];
  readonly #recent: string[] = [];
  readonly #logger: Logger = {
    isEnabled: (name) => name === 'api',
    log: (_name, _severity, message) => this.#recordOperation(message),
  };

  /** Retain context acquisition before either page creation or caller-owned navigation can await. */
  async openPage(browser: GimpFixtureBrowser<Page>): Promise<Page> {
    if (this.#context || this.#retired) throw new Error('GIMP page lifetime is already used.');
    this.#phase = 'acquiring context';
    this.#context = browser.newContext({ logger: this.#logger });
    const context = await this.#context;
    if (this.#retired) throw new Error('GIMP page lifetime retired during context acquisition.');
    this.#phase = 'opening page';
    const page = await context.newPage();
    if (this.#retired) throw new Error('GIMP page lifetime retired during page acquisition.');
    this.#page = page;
    this.#phase = 'ready';
    return page;
  }

  /** Return a bounded copy before cleanup changes the operation that caused a failure. */
  diagnostics(): GimpFixtureDiagnostics {
    return { phase: this.#phase, pending: [...this.#pending], recent: [...this.#recent] };
  }

  /** Cancel pending page work by closing its context, including contexts acquired after retirement. */
  retire(disposePage: (page: Page) => Promise<void>): Promise<void> {
    this.#retired = true;
    this.#retirement ??= this.#release(disposePage);
    return this.#retirement;
  }

  /** Attempt product disposal and context closure independently, preserving every cleanup cause. */
  async #release(disposePage: (page: Page) => Promise<void>): Promise<void> {
    this.#phase = 'retiring';
    try {
      const context = await this.#context;
      if (context) {
        const failures: unknown[] = [];
        if (this.#page !== undefined) {
          try {
            await disposePage(this.#page);
          } catch (error) {
            failures.push(error);
          }
        }
        try {
          await context.close();
        } catch (error) {
          failures.push(error);
        }
        if (failures.length === 1) throw failures[0];
        if (failures.length) throw new AggregateError(failures, 'GIMP page cleanup failed');
      }
      this.#phase = 'retired';
    } catch (error) {
      this.#phase = 'retirement failed';
      throw error;
    }
  }

  /** Retain API names and outcomes from Playwright's supported logger without recording payloads. */
  #recordOperation(message: string | Error): void {
    if (typeof message !== 'string') return;
    const operation = /^(=>|<=) (.+) (started|succeeded|failed)$/.exec(message);
    if (!operation) return;
    const name = operation[2]!.slice(0, 160);
    if (operation[1] === '=>') {
      this.#pending.push(name);
      if (this.#pending.length > 16) this.#pending.shift();
    } else {
      const pending = this.#pending.indexOf(name);
      if (pending !== -1) this.#pending.splice(pending, 1);
    }
    this.#recent.push(`${name} ${operation[3]}`);
    if (this.#recent.length > 16) this.#recent.shift();
  }
}
