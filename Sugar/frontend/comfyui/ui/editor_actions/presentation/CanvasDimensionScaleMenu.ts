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
/** Present explicit sizing choices inside the shared context-menu lifetime. */
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import { CanvasCancellation, canvasSizingProposal } from '../../core/embedded/CanvasRuntime.js';
import type {
  CanvasDimensionChoice,
  CanvasDimensionConfiguration,
} from '../../../../../integrations/contracts/CanvasGeometry.js';
import type { CanvasDimensions } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasDimensionAction,
  DimensionSizingPort,
} from '../application/CanvasDimensionCommands.js';

export type ExecuteCanvasDimension = (
  action: CanvasDimensionAction,
  configure?: DimensionSizingPort,
  cancellation?: CanvasCancellation,
) => Promise<void>;

/** Retain the factor input while discovery changes only menu values and available output paths. */
export class CanvasDimensionScaleMenu {
  #request: CanvasDimensionConfiguration | null = null;
  #scale = '1';
  #output: string | null = null;
  #message = 'Reading canvas dimensions…';
  #pending: ((choice: CanvasDimensionChoice) => void) | null = null;
  #refresh: () => void = () => {};
  #close: () => void = () => {};
  #submitted = false;

  /** Borrow the application command without owning graph, geometry or probe policy. */
  constructor(
    private readonly action: CanvasDimensionAction,
    private readonly execute: ExecuteCanvasDimension,
    private readonly run: (operation: Promise<void>) => void,
  ) {}

  /** Build one stable submenu whose dynamic labels never replace its input. */
  entries(): readonly ContextMenuEntry[] {
    const owner = this;
    return [
      {
        label: 'Scale factor',
        get disabled() {
          return owner.#pending === null;
        },
        input: {
          get value() {
            return owner.#scale;
          },
          minimum: 0.000001,
          maximum: 1000,
          step: 'any',
          change: (value) => {
            this.#scale = value;
          },
        },
      },
      {
        get label() {
          const output = owner.#request?.outputs.find((item) => item.id === owner.#output);
          return output ? `Output: ${output.label}` : 'Choose output path';
        },
        get disabled() {
          return !owner.#pending || !owner.#request?.outputs.length;
        },
        submenuPlacement: 'below',
        get children() {
          return (
            owner.#request?.outputs.map((output) => ({
              label: output.label,
              get checked() {
                return owner.#output === output.id;
              },
              keepOpen: 'parent' as const,
              select: () => {
                owner.#output = output.id;
                owner.#refresh();
              },
            })) ?? []
          );
        },
      },
      {
        label: 'Discover scale factor',
        keepOpen: true,
        get disabled() {
          return !owner.#pending || Boolean(owner.#request?.outputs.length && !owner.#output);
        },
        select: () => this.#choose('discover'),
      },
      {
        get label() {
          const size = owner.#size();
          return size
            ? `${owner.#request?.direction === 'to-canvas' ? 'Canvas' : 'Size'}: ${size.width} × ${size.height} px`
            : owner.#pending
              ? 'Enter a positive scale factor ≤ 1000'
              : owner.#message;
        },
        disabled: true,
      },
      {
        label: 'Apply dimensions',
        keepOpen: true,
        get disabled() {
          return !owner.#pending || !owner.#size();
        },
        select: () => {
          this.#submitted = true;
          this.#choose('apply');
          this.#close();
        },
      },
    ];
  }

  /** Cancel abandoned interactions, while an explicit Apply finishes its captured command. */
  readonly opened = (refresh: () => void, close: () => void): (() => void) => {
    const cancellation = new CanvasCancellation();
    this.#request = null;
    this.#scale = '1';
    this.#output = null;
    this.#message = 'Reading canvas dimensions…';
    this.#submitted = false;
    this.#refresh = refresh;
    this.#close = close;
    const configure: DimensionSizingPort = (request) => {
      if (cancellation.cancelled())
        return Promise.resolve({ choice: 'cancel', scale: request.scale, output: request.output });
      this.#request = request;
      this.#scale = request.scale;
      this.#output =
        request.output ?? (request.outputs.length === 1 ? (request.outputs[0]?.id ?? null) : null);
      return new Promise((resolve) => {
        const stop = cancellation.subscribe(() => {
          this.#pending = null;
          resolve({ choice: 'cancel', scale: request.scale, output: request.output });
        });
        this.#pending = (choice) => {
          stop();
          resolve(choice);
        };
        refresh();
      });
    };
    this.run(
      this.execute(this.action, configure, cancellation).catch((error: unknown) => {
        if (!cancellation.cancelled()) {
          this.#pending = null;
          this.#message = error instanceof Error ? error.message : 'Dimensions are unavailable.';
          refresh();
          throw error;
        }
      }),
    );
    return () => {
      if (!this.#submitted) cancellation.cancel();
    };
  };

  /** Reuse the same constrained arithmetic that the command applies. */
  #size(): CanvasDimensions | null {
    if (!this.#request || !this.#pending) return null;
    try {
      return canvasSizingProposal(this.#request, this.#scale);
    } catch {
      return null;
    }
  }

  /** Resume application discovery or mutation only after the user selects the corresponding command. */
  #choose(choice: 'apply' | 'discover'): void {
    const pending = this.#pending;
    if (!pending || !this.#request) return;
    const scale = this.#size() ? this.#scale : this.#request.scale;
    this.#pending = null;
    this.#message = choice === 'discover' ? 'Discovering scale factor…' : 'Applying dimensions…';
    pending({ choice, scale, output: this.#output });
    this.#refresh();
  }
}
