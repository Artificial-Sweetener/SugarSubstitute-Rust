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
/** Keep the local connection shell usable independently of the server-hosted SugarCubes interface. */
export interface ConnectionState {
  readonly phase: 'connecting' | 'disconnected' | 'connected';
  readonly address: string;
  readonly message: string;
}

/** Supply the prototype's native connection operation without leaking its host framework. */
export interface CompanionConnectionPort {
  connect(publish: (state: ConnectionState) => void): Promise<void>;
  report(error: unknown): void;
}

/** Coalesce retries and fence late results after the local shell is retired. */
export class CompanionConnection {
  #pending = false;
  #disposed = false;
  #attempt = 0;
  /** Keep presentation and native connection mechanics under their separate owners. */
  constructor(
    private readonly port: CompanionConnectionPort,
    private readonly present: (state: ConnectionState) => void,
  ) {}
  /** Try the configured server once without inventing another canvas or execution workflow. */
  async retry(): Promise<void> {
    if (this.#pending || this.#disposed) return;
    this.#pending = true;
    const attempt = ++this.#attempt;
    this.present({ phase: 'connecting', address: '', message: 'Connecting to ComfyUI…' });
    try {
      await this.port.connect((state) => {
        if (this.#acceptsResults() && this.#attempt === attempt) this.present(state);
      });
    } catch (error: unknown) {
      this.port.report(error);
      if (this.#acceptsResults())
        this.present({
          phase: 'disconnected',
          address: '',
          message: 'The companion could not connect. Check ComfyUI, then retry.',
        });
    } finally {
      this.#pending = false;
    }
  }
  /** Prevent server or native responses from reviving a closed shell. */
  dispose(): void {
    this.#disposed = true;
  }
  /** Re-read the lifetime after asynchronous native work, which may outlive this shell. */
  #acceptsResults(): boolean {
    return !this.#disposed;
  }
}
