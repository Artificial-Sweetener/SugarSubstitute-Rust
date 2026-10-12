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
/** Compose reusable floating controls independently of Comfy and image rendering. */

import { computed, defineComponent, h, type PropType } from 'vue';
import type {
  CanvasCapabilities,
  CanvasCommand,
  CanvasViewSnapshot,
} from '../../domain/CanvasViewPort.js';
import { CanvasCompareControls } from './CanvasCompareControls.js';
import { CanvasSourceTabs } from './CanvasSourceTabs.js';
import { CanvasToolbar } from './CanvasToolbar.js';
import { CanvasWipeSeparator } from './CanvasWipeSeparator.js';
import { canvasNavigationSnapshot } from './CanvasNavigationSnapshot.js';

/** Render one view's status and navigation over any qualified surface. */
export const CanvasOverlay = defineComponent({
  name: 'CanvasOverlay',
  props: {
    snapshot: { type: Object as PropType<CanvasViewSnapshot>, required: true },
    capabilities: { type: Object as PropType<CanvasCapabilities>, required: true },
    send: { type: Function as PropType<(command: CanvasCommand) => void>, required: true },
    announcement: { type: String, default: '' },
  },
  /** Compose reported surface status with capability-gated control groups. */
  setup(props) {
    const navigation = computed<CanvasViewSnapshot>((previous) =>
      canvasNavigationSnapshot(props.snapshot, previous),
    );
    return () => {
      const snapshot = props.snapshot;
      const zoom = snapshot.zoom;
      const dimensions = zoom.dimensions;
      const label =
        zoom.mode === 'fit'
          ? 'Fit'
          : zoom.mode === 'native'
            ? '1:1'
            : `${Math.round(zoom.physicalScale * 100)}%`;
      return h('div', { class: 'sc-output-overlay', 'data-canvas-view': snapshot.address.viewId }, [
        snapshot.view.mode === 'image' &&
        snapshot.view.comparison &&
        props.capabilities.commands.includes('wipe')
          ? h(CanvasWipeSeparator, { snapshot, send: props.send })
          : null,
        h('div', { class: 'sc-output-navigation' }, [
          h(CanvasToolbar, {
            snapshot: navigation.value,
            capabilities: props.capabilities,
            send: props.send,
          }),
          snapshot.view.mode === 'image' && snapshot.view.comparison
            ? h(CanvasCompareControls, {
                snapshot: navigation.value,
                capabilities: props.capabilities,
                send: props.send,
              })
            : null,
        ]),
        h('div', { class: 'sc-output-status' }, [
          h('span', { 'aria-label': 'Zoom' }, label),
          dimensions ? h('span', `${dimensions.width} × ${dimensions.height}`) : null,
          zoom.secondaryPhysicalScale && zoom.secondaryDimensions
            ? h(
                'span',
                { 'aria-label': 'B display scale' },
                `B: ${Math.round(zoom.secondaryPhysicalScale.x * 100)}% × ${Math.round(zoom.secondaryPhysicalScale.y * 100)}%; ${zoom.secondaryDimensions.width} × ${zoom.secondaryDimensions.height}`,
              )
            : null,
          snapshot.preview ? h('span', 'Live preview') : null,
          snapshot.connected ? null : h('span', 'Disconnected'),
          h('span', snapshot.resource.message),
          snapshot.resource.canRetry && props.capabilities.commands.includes('retry')
            ? h(
                'button',
                { type: 'button', onClick: () => props.send({ type: 'retry' }) },
                'Retry image',
              )
            : null,
          snapshot.resource.canAdmitLargeImage &&
          props.capabilities.commands.includes('admit-large-image')
            ? h(
                'button',
                { type: 'button', onClick: () => props.send({ type: 'admit-large-image' }) },
                'Load large image',
              )
            : null,
          snapshot.selected?.mediaKind !== undefined &&
          snapshot.selected.mediaKind !== 'image' &&
          props.capabilities.commands.includes('open-media')
            ? h(
                'button',
                {
                  type: 'button',
                  onClick: () => props.send({ type: 'open-media', target: 'primary' }),
                },
                'Open media',
              )
            : null,
        ]),
        h(
          'span',
          {
            class: 'sc-output-announcement',
            role: 'status',
            'aria-live': 'polite',
            'aria-atomic': 'true',
          },
          props.announcement,
        ),
        props.capabilities.commands.includes('select-source')
          ? h(CanvasSourceTabs, { snapshot: navigation.value, send: props.send })
          : null,
      ]);
    };
  },
});
