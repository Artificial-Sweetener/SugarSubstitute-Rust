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
/** One finite passive scene lease publishes supported CM opacity and owns every external paint resource. */
import { requireValue } from '../../core/src/required.js';
import { releaseOwnedMotionResource, type OwnedMotionResource } from './reorder-motion-owned.js';
import { ReorderMotionObservation } from './reorder-motion-observation.js';
import { snapshotPresentation } from './native-snapshot-policy.js';
import { prepareReorderVisibilityReceipt } from './reorder-motion-visibility-receipt.js';
import type { EditorView } from '@codemirror/view';
import type { ReorderPointerPaint } from './reorder-preview-contracts.js';
import type { NativeReorderCapture } from './reorder-command.js';
import { reorderPreview } from './reorder-preview.js';
import { reorderPaintIssueName } from './reorder-paint-identity.js';
import { setReorderMotionMask } from './reorder-motion-mask.js';
import { ReorderMotionStyles, type ReorderMotionHoverPolicy } from './reorder-motion-styles.js';
import type { ReorderLandingPublication } from './reorder-landing-paint.js';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
/** Semantic and issued-generation authority remains with the native presentation caller. */
interface ReorderMotionResourcePorts {
  observe<T>(action: () => T): T;
}
/** No source buffer, request lane or gesture generation is retained by this DOM lease. */
export class ReorderMotionResource {
  private readonly view: EditorView;
  private readonly ports: ReorderMotionResourcePorts;
  private styles: ReorderMotionStyles | null = null;
  private owned: OwnedMotionResource | null = null;
  private handoff: object | null = null;
  /** Bind the canonical view and its existing unexpected-cause boundary. */
  constructor(view: EditorView, ports: ReorderMotionResourcePorts) {
    this.view = view;
    this.ports = ports;
  }
  /** The borrowed scene is valid only while its exact mask and mounted paint publication remain current. */
  get scene(): ReorderMotionScene | null {
    return this.owned?.scene ?? null;
  }
  /** Constant-time admission checks perform no DOM layout or semantic traversal. */
  current(): boolean {
    const owned = this.owned;
    if (!owned || owned.released || this.handoff) return false;
    const admitted = owned.admitted() && owned.lease();
    return (
      admitted &&
      !owned.released &&
      this.owned === owned &&
      this.styles === owned.styles &&
      owned.styles.available &&
      (owned.landing === null || owned.landing.current()) &&
      owned.mask !== null &&
      this.view.state.field(reorderPreview, false)?.mask === owned.mask &&
      this.handoff === null
    );
  }
  /** Whole rules may retain one gate only across the same primary hold, immutable anchor and valid native Stage. */
  private previous(
    scene: ReorderMotionScene,
    capture: NativeReorderCapture,
    hover: ReorderMotionHoverPolicy,
    admitted: () => boolean,
    lease: () => boolean,
    publishable: () => boolean,
  ): {
    readonly owned: OwnedMotionResource;
    readonly stage: HTMLElement;
    readonly pointer: ReorderPointerPaint;
  } | null {
    if (this.handoff) return null;
    const old = this.owned,
      state = this.view.state.field(reorderPreview, false),
      pointer = state?.pointer;
    if (
      !old?.scene.whole ||
      !scene.whole ||
      hover !== 'whole' ||
      !state?.value ||
      !pointer ||
      pointer.mode !== 'stage' ||
      !old.pointer ||
      old.pointer.hold !== pointer.hold ||
      old.pointer.anchor !== pointer.anchor ||
      !Object.isFrozen(pointer.anchor) ||
      state.paint !== pointer.anchor ||
      old.mask?.coverage !== 'value' ||
      old.mask.capture !== capture ||
      old.mask.scope !== old.styles.scope ||
      old.styles.root !== this.view.root ||
      !this.current()
    )
      return null;
    const stage = old.scene.node.parentElement;
    if (
      !stage ||
      scene.node.parentElement !== stage ||
      !scene.node.isConnected ||
      !this.view.dom.contains(scene.node) ||
      !publishable() ||
      !admitted() ||
      !lease() ||
      this.owned !== old ||
      !this.current()
    )
      return null;
    return { owned: old, stage, pointer };
  }
  /** Publish one exact mask/receipt/reveal pipeline, retaining a compatible whole gate without native gate writes. */
  adopt(
    scene: ReorderMotionScene,
    lease: () => boolean,
    capture: NativeReorderCapture,
    hover: ReorderMotionHoverPolicy,
    admitted: () => boolean,
    changed: () => void,
    publishable: () => boolean,
    fallback: () => void,
    prepareLanding: (() => ReorderLandingPublication | null) | null,
  ): boolean {
    const entry = this.owned;
    let previous: ReturnType<ReorderMotionResource['previous']>,
      candidateRetired = false;
    try {
      previous = this.previous(scene, capture, hover, admitted, lease, publishable);
      if (!publishable() || !admitted() || !lease()) {
        candidateRetired = true;
        scene.destroy();
        return false;
      }
      if (!previous) this.release();
    } catch (primary) {
      const failures: unknown[] = [primary];
      for (const release of [
        () => {
          if (this.owned === entry) fallback();
        },
        () => {
          if (this.owned === entry) this.release();
        },
        () => {
          if (!candidateRetired && this.scene !== scene) scene.destroy();
        },
      ])
        try {
          release();
        } catch (cleanup) {
          failures.push(cleanup);
        }
      if (failures.length > 1)
        throw new AggregateError(failures, 'reorder-motion-acquisition-failed');
      throw primary;
    }
    let styles = previous?.owned.styles ?? this.styles,
      sceneRetired = false,
      landing: ReorderLandingPublication | null = null;
    try {
      if (styles && (!styles.available || styles.root !== this.view.root)) {
        this.styles = null;
        styles.retire();
        styles = null;
      }
      if (!publishable() || !admitted() || !lease()) {
        sceneRetired = true;
        scene.destroy();
        return false;
      }
      if (!styles) this.styles = styles = new ReorderMotionStyles(this.view);
      landing = prepareLanding?.() ?? null;
      if (scene.whole && !landing) {
        sceneRetired = true;
        scene.destroy();
        return false;
      }
    } catch (primary) {
      if (sceneRetired) throw primary;
      try {
        scene.destroy();
      } catch (cleanup) {
        throw new AggregateError([primary, cleanup], 'reorder-motion-style-acquisition-failed');
      }
      throw primary;
    }
    const issue = {},
      handoff = {},
      owned: OwnedMotionResource = {
        issue,
        released: false,
        pointer: this.view.state.field(reorderPreview, false)?.pointer ?? null,
        mask: null,
        scene,
        styles,
        lease,
        admitted,
        observation: null,
        landing,
      };
    owned.observation = this.createObservation(owned, capture, changed);
    let committed = !previous,
      complete = false,
      landingCommitted = false;
    if (previous) this.handoff = handoff;
    else this.owned = owned;
    /** Exact acquisition and retained Stage leases fence every delegated native continuation. */
    const retainedCurrent = (): boolean => {
      const accepted =
        !owned.released &&
        (!landingCommitted || !!landing?.current()) &&
        admitted() &&
        lease() &&
        (!previous || committed || (previous.owned.admitted() && previous.owned.lease()));
      return (
        accepted &&
        this.styles === styles &&
        styles.available &&
        this.owned ===
          (committed ? owned : requireValue(previous, 'reorder-motion-resource').owned) &&
        (!previous || this.handoff === (complete ? null : handoff)) &&
        (!previous ||
          (scene.node.parentElement === previous.stage &&
            (committed || previous.owned.scene.node.parentElement === previous.stage))) &&
        this.owned ===
          (committed ? owned : requireValue(previous, 'reorder-motion-resource').owned) &&
        (!previous || this.handoff === (complete ? null : handoff))
      );
    };
    const current = (): boolean =>
      ((previous && committed) || publishable()) &&
      retainedCurrent() &&
      (!previous || this.view.state.field(reorderPreview, false)?.pointer === previous.pointer) &&
      ((!owned.mask && !previous) ||
        this.view.state.field(reorderPreview, false)?.mask ===
          (owned.mask ?? previous?.owned.mask)) &&
      retainedCurrent();
    /** Same-hold readonly proposals may keep accepted physical paint while their later READ is pending. */
    const compatible = (): boolean => {
      const state = this.view.state.field(reorderPreview, false);
      return (
        !!previous &&
        retainedCurrent() &&
        state?.value?.capture === capture &&
        state.pointer === previous.pointer &&
        state.paint === previous.pointer.anchor &&
        retainedCurrent()
      );
    };
    /** A rejected acquisition restores canonical paint before retiring each captured resource independently. */
    const reject = (reason: 'rejected' | 'failed' = 'rejected'): void => {
      const failures: unknown[] = [],
        owns = () => (previous ? this.handoff === handoff : this.owned === owned);
      if (
        reason === 'rejected' &&
        previous &&
        !committed &&
        !publishable() &&
        owns() &&
        compatible()
      ) {
        let mask = owned.mask ?? previous.owned.mask;
        const rollbackCurrent = (): boolean =>
          owns() && compatible() && this.view.state.field(reorderPreview, false)?.mask === mask;
        const receipt = prepareReorderVisibilityReceipt(
          this.view,
          requireValue(previous.owned.mask, 'reorder-motion-resource'),
          rollbackCurrent,
        );
        if (
          receipt &&
          rollbackCurrent() &&
          requireValue(owned.observation, 'reorder-motion-resource').cleanPending(rollbackCurrent)
        ) {
          this.view.dispatch({
            effects: setReorderMotionMask.of(
              requireValue(previous.owned.mask, 'reorder-motion-resource'),
            ),
            annotations: snapshotPresentation.of(true),
          });
          mask = this.view.state.field(reorderPreview, false)?.mask ?? null;
          if (
            mask?.issue === previous.owned.issue &&
            mask.scope === styles.scope &&
            rollbackCurrent() &&
            receipt(owned.observation?.takeContentRecords() ?? null) &&
            rollbackCurrent()
          ) {
            previous.owned.mask = mask;
            if (this.handoff === handoff) this.handoff = null;
            this.releaseOwned(owned);
            return;
          }
        }
      }
      const captured = owns();
      if (captured) {
        try {
          fallback();
        } catch (cause) {
          failures.push(cause);
        }
        if (owns()) {
          if (previous) this.handoff = null;
          if (this.owned === owned || this.owned === previous?.owned) this.owned = null;
        }
      }
      for (const release of [
        ...(!committed || captured ? [() => this.releaseOwned(owned)] : []),
        ...(previous && captured ? [() => this.releaseOwned(previous.owned)] : []),
      ])
        try {
          release();
        } catch (cause) {
          failures.push(cause);
        }
      if (failures.length === 1) throw failures[0];
      if (failures.length)
        throw new AggregateError(failures, 'reorder-motion-resource-retirement-failed');
    };
    try {
      const ids = scene.whole
        ? scene.targets.map((target) => target.id)
        : scene.chips.map((chip) => chip.id);
      if (
        previous
          ? !requireValue(previous.owned.observation, 'reorder-motion-resource').cleanPending(
              current,
            )
          : !styles.prepare(ids, hover, current)
      ) {
        reject();
        return false;
      }
      if (!current()) {
        reject();
        return false;
      }
      if (!scene.node.isConnected || !this.view.dom.contains(scene.node))
        throw new Error('reorder-motion-external-context-unavailable');
      if (!current()) {
        reject();
        return false;
      }
      requireValue(owned.observation, 'reorder-motion-resource').start();
      const pending = owned.observation?.takeContentRecords() ?? [];
      if (
        !current() ||
        pending.length ||
        (previous &&
          !requireValue(previous.owned.observation, 'reorder-motion-resource').cleanPending(
            current,
          ))
      ) {
        if (pending.length) changed();
        reject();
        return false;
      }
      const nextMask = {
          issue,
          scope: styles.scope,
          capture,
          ids,
          ...(scene.whole ? { coverage: 'value' as const } : {}),
        },
        receipt = prepareReorderVisibilityReceipt(this.view, nextMask, current);
      if (!receipt || !current()) {
        reject();
        return false;
      }
      this.view.dispatch({
        effects: setReorderMotionMask.of(nextMask),
        annotations: snapshotPresentation.of(true),
      });
      owned.mask = this.view.state.field(reorderPreview, false)?.mask ?? null;
      if (owned.mask?.issue !== issue || owned.mask.scope !== styles.scope || !current()) {
        reject();
        return false;
      }
      if (!receipt(owned.observation?.takeContentRecords() ?? null) || !current()) {
        if ((this.owned === owned || (previous && this.handoff === handoff)) && current())
          changed();
        reject();
        return false;
      }
      if (
        previous &&
        !requireValue(owned.observation, 'reorder-motion-resource').cleanPending(current)
      ) {
        reject();
        return false;
      }
      if (
        previous
          ? !styles.transfer(previous.owned.issue, issue, ids, current)
          : !styles.activate(issue, current)
      ) {
        reject();
        return false;
      }
      if (previous) {
        // Publish matching JS ownership before any captured old native cleanup can reenter.
        this.owned = owned;
        committed = true;
        if (!requireValue(owned.observation, 'reorder-motion-resource').cleanPending(current)) {
          reject();
          return false;
        }
      }
      // Matching contour ownership transfers before old cleanup, reveal and the first endpoint/clock publication.
      if (landing) {
        if (!landing.commit(current)) {
          reject();
          return false;
        }
        landingCommitted = true;
      }
      if (previous) this.releaseOwned(previous.owned);
      if (!current()) {
        reject();
        return false;
      }
      if (scene.whole) {
        requireValue(scene.reveal, 'reorder-motion-resource')();
        if (!current()) {
          reject();
          return false;
        }
        requireValue(
          scene.publishEndpoint,
          'reorder-motion-resource',
        )(reorderPaintIssueName(issue));
        if (
          !current() ||
          !requireValue(owned.observation, 'reorder-motion-resource').cleanPending(current)
        ) {
          reject();
          return false;
        }
      }
      if (previous && this.handoff === handoff) this.handoff = null;
      complete = true;
      return true;
    } catch (primary) {
      const failures: unknown[] = [primary];
      try {
        reject('failed');
      } catch (cleanup) {
        failures.push(cleanup);
      }
      if (this.styles === styles && !this.owned) {
        this.styles = null;
        try {
          styles.retire();
        } catch (cleanup) {
          failures.push(cleanup);
        }
      }
      if (failures.length > 1)
        throw new AggregateError(failures, 'reorder-motion-publication-failed');
      throw primary;
    }
  }
  /** Persistent observation ports capture this acquisition only, never an adoption or previous handoff environment. */
  private createObservation(
    owned: OwnedMotionResource,
    capture: NativeReorderCapture,
    changed: () => void,
  ): ReorderMotionObservation {
    return new ReorderMotionObservation(this.view, capture, {
      owned: () => this.owned === owned,
      current: () => {
        const accepted = !owned.released && owned.admitted() && owned.lease();
        return (
          accepted &&
          !owned.released &&
          this.owned === owned &&
          this.styles === owned.styles &&
          owned.styles.available
        );
      },
      observe: (action) => this.ports.observe(action),
      changed,
    });
  }
  /** Exact-issued cleanup attempts every resource and preserves original ordered causes. */
  release(): void {
    const owned = this.owned;
    this.owned = null;
    this.handoff = null;
    if (owned) this.releaseOwned(owned);
  }
  /** Captured old cleanup cannot release the currently adopted issue or its scene. */
  private releaseOwned(owned: OwnedMotionResource): void {
    const failures = releaseOwnedMotionResource(this.view, owned, (action) =>
      this.ports.observe(action),
    );
    if (this.styles === owned.styles && !owned.styles.available) this.styles = null;
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'reorder-motion-resource-retirement-failed');
  }
  /** Cancel/context retirement removes the captured finite sheet; reentrant newer owners retain their own resources. */
  retire(): void {
    const styles = this.styles;
    this.styles = null;
    const failures: unknown[] = [];
    for (const release of [() => this.release(), () => styles?.retire()])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'reorder-motion-context-retirement-failed');
  }
}
