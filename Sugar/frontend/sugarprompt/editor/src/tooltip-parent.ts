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
/** Native lifetime owns shared inherited tooltip scope and finite manual top-layer publication. */
import {
  showTooltip,
  tooltips,
  ViewPlugin,
  type EditorView,
  type Rect,
  type ViewUpdate,
} from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import type { TooltipTransport, TooltipTransportFactory } from './tooltip-transport.js';

/** Native tooltips share a four-CSS-pixel viewport safe edge; each tiny axis drops its inset to retain available space. */
export function nativeTooltipSpace(view: EditorView): Rect {
  const viewport = view.dom.ownerDocument.documentElement;
  const width = viewport.clientWidth,
    height = viewport.clientHeight;
  const horizontal = width > 8 ? 4 : 0,
    vertical = height > 8 ? 4 : 0;
  return {
    left: horizontal,
    right: width - horizontal,
    top: vertical,
    bottom: height - vertical,
  };
}

/** One current native view delegates only its supported tooltip facet membership to the shared DOM owner. */
export interface NativeTooltipLease {
  /** Observe the current immutable facet; unchanged active membership performs no native calls. */
  update(update: ViewUpdate): void;
  /** Retire only this native view's generation, never a replacement view's publication. */
  destroy(): void;
}

/** CodeMirror owns tooltip children and placement; host ancestry owns live CSS and the parent owns one manual top-layer entry. */
export class NativeTooltipParent {
  readonly dom: HTMLElement;
  readonly configuration: Extension;
  private lease: object | null = null;
  private active = false;
  private published = false;
  private changing = false;
  private queued = false;
  private disposed = false;
  private readonly transport: TooltipTransport;

  /** Native detach/reattach may close a manual popover independently of immutable tooltip membership. */
  private readonly toggled = (): void => {
    if (this.disposed) return;
    try {
      this.published = this.dom.matches(':popover-open');
      this.reconcile();
    } catch (cause) {
      this.fail(cause);
    }
  };

  /** Mount literal context inside the actual host and its ShadowRoot; an empty parent never enters the top layer. */
  constructor(parent: HTMLElement, label: string, createTransport: TooltipTransportFactory) {
    this.dom = parent.ownerDocument.createElement('div');
    this.dom.className = 'sp-native-tooltip-parent';
    this.transport = createTransport(this.dom);
    this.dom.setAttribute('role', 'region');
    this.dom.setAttribute('aria-label', `${label} editor overlays`);
    Object.assign(this.dom.style, {
      position: 'fixed',
      inset: '0',
      width: 'auto',
      height: 'auto',
      maxWidth: 'none',
      maxHeight: 'none',
      margin: '0',
      padding: '0',
      border: '0',
      background: 'transparent',
      color: 'inherit',
      overflow: 'visible',
      pointerEvents: 'none',
    });
    const style = parent.ownerDocument.createElement('style');
    style.textContent =
      '.sp-native-tooltip-parent::backdrop{background:transparent;pointer-events:none}.sp-native-tooltip-parent .cm-tooltip{pointer-events:auto}';
    this.configuration = [
      tooltips({ parent: this.dom, tooltipSpace: nativeTooltipSpace }),
      ViewPlugin.define((view) => this.bind(view)),
    ];
    try {
      this.dom.addEventListener('toggle', this.toggled);
      this.dom.append(style);
      parent.append(this.dom);
    } catch (cause) {
      this.fail(cause);
    }
  }

  /** Bind the actual native-view construction path; replacement acquisition invalidates every prior lease. */
  bind(view: EditorView): NativeTooltipLease {
    const lease = {};
    if (this.disposed) throw new Error('NATIVE_TOOLTIP_PARENT_RETIRED');
    this.lease = lease;
    /** Sample this view facet once, fence external read reentry, and publish only its still-current membership. */
    const project = (): void => {
      if (this.disposed || this.lease !== lease) return;
      const active = view.state.facet(showTooltip).some((tooltip) => tooltip !== null);
      if (this.disposed || this.lease !== lease) return;
      this.active = active;
      this.reconcile();
    };
    try {
      project();
    } catch (cause) {
      const failures: unknown[] = [cause];
      try {
        this.release(lease);
      } catch (cleanup) {
        failures.push(cleanup);
      }
      this.report(failures);
    }
    return {
      /** Native state remains the only tooltip membership authority. */
      update: (_update: ViewUpdate): void => project(),
      /** An old view can release no state retained by the new current view. */
      destroy: (): void => this.release(lease),
    };
  }

  /** Release the current generation's derived membership without retaining its editor view. */
  private release(lease: object): void {
    if (this.disposed || this.lease !== lease) return;
    this.lease = null;
    this.active = false;
    this.reconcile();
  }

  /** One native operation checks cancelable/reentrant publication; ordinary unchanged membership stays entirely inert. */
  private reconcile(): void {
    if (this.disposed || this.active === this.published) return;
    const lease = this.lease,
      active = this.active;
    const connected = !active || this.dom.isConnected;
    if (this.disposed || this.lease !== lease || this.active !== active) {
      this.defer();
      return;
    }
    if (!connected) return;
    if (this.changing) {
      this.defer();
      return;
    }
    this.changing = true;
    const failures: unknown[] = [];
    try {
      if (active) this.transport.show();
      else this.transport.hide();
      this.published = this.dom.matches(':popover-open');
    } catch (cause) {
      failures.push(cause);
    }
    this.changing = false;
    if (failures.length) {
      this.retire(failures);
      this.report(failures);
    }
    if (this.disposed) return;
    if (this.lease !== lease || this.active !== active) {
      this.defer();
      return;
    }
    if (this.published !== active) this.fail(new Error('NATIVE_TOOLTIP_PUBLICATION_CANCELLED'));
  }

  /** HTML rejects nested show/hide algorithms; one finite coalesced microtask reconciles only the latest generation after beforetoggle reentry. */
  private defer(): void {
    if (this.queued || this.disposed) return;
    this.queued = true;
    try {
      queueMicrotask(() => {
        this.queued = false;
        if (!this.disposed) this.reconcile();
      });
    } catch (cause) {
      this.queued = false;
      this.fail(cause);
    }
  }

  /** Invalidate every lease before independently attempting native top-layer and DOM retirement. */
  private retire(failures: unknown[]): void {
    if (this.disposed) return;
    this.disposed = true;
    this.lease = null;
    this.active = false;
    // Removing a connected popover retires its top-layer entry even during an in-progress show/hide algorithm.
    if (!this.changing) {
      try {
        if (this.dom.matches(':popover-open')) this.transport.hide();
      } catch (cause) {
        failures.push(cause);
      }
    }
    try {
      this.transport.dispose();
      this.dom.removeEventListener('toggle', this.toggled);
    } catch (cause) {
      failures.push(cause);
    }
    try {
      this.dom.remove();
    } catch (cause) {
      failures.push(cause);
    }
  }

  /** Unexpected acquisition/publication failures release owned resources while retaining the raw primary first. */
  private fail(cause: unknown): never {
    const failures: unknown[] = [cause];
    this.retire(failures);
    this.report(failures);
    throw cause;
  }

  /** Count-based reporting preserves null/undefined causes and ordered independent cleanup failures. */
  private report(failures: readonly unknown[]): void {
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, 'Native tooltip parent failed');
  }

  /** Controller teardown runs after native-view disposal; queued reentrant reconciliation becomes inert. */
  dispose(): void {
    const failures: unknown[] = [];
    this.retire(failures);
    this.report(failures);
  }
}
