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
/** Exercise the real progress view and native visibility rules in an isolated browser. */
import { GenerationProgressView } from '../../../frontend/comfyui/ui/surface/frame/GenerationProgressView.js';
import { substituteFrameCss } from '../../../frontend/comfyui/ui/surface/frame/SubstituteFrameStyles.js';
import { SubstituteToolbarLayout } from '../../../frontend/comfyui/ui/surface/frame/SubstituteToolbarLayout.js';

declare global {
  interface Window {
    progressBrowserProof: {
      update(visible: boolean, workflow: number, sampler: number, dark: boolean): void;
      mode(enabled: boolean): void;
      queueExpanded(expanded: boolean): void;
      toolbar(present: boolean): void;
    };
  }
}

const style = document.createElement('style');
style.textContent = substituteFrameCss;
document.head.append(style);
const frame = document.createElement('header');
frame.className = 'sc-substitute-frame';
Object.assign(frame.style, { width: '720px', height: '42px', position: 'relative' });
frame.style.setProperty('--sc-accent-color', '#b63acb');
document.body.append(frame);
const bars = new GenerationProgressView(document);
frame.append(bars.element);
const native = document.createElement('div');
native.id = 'native-progress';
const fill = document.createElement('div');
fill.dataset.testid = 'linear-progress-overall';
native.append(fill);
document.body.append(native);
const queue = document.createElement('button');
queue.dataset.scGenerationQueue = '';
queue.setAttribute('aria-pressed', 'false');
const overlay = document.createElement('div');
overlay.dataset.testid = 'queue-progress-overlay';
document.body.append(queue, overlay);
document.documentElement.setAttribute('data-sc-substitute-frame', '');

// Mirror Comfy's top menu row: the workflow selector and actionbar share one toolbar line.
const line = document.createElement('div');
line.id = 'toolbar-line';
Object.assign(line.style, { width: '720px' });
const workflowColumn = document.createElement('div');
const workflowActions = document.createElement('button');
workflowActions.setAttribute('aria-label', 'Workflow actions');
workflowColumn.append(workflowActions);
const actionColumn = document.createElement('div');
const actionbar = document.createElement('div');
actionbar.className = 'actionbar-container';
actionColumn.append(actionbar);
line.append(workflowColumn, actionColumn);
const popup = document.createElement('div');
popup.id = 'later-popup';
Object.assign(popup.style, {
  position: 'absolute',
  left: '0',
  top: '60px',
  width: '200px',
  height: '40px',
  zIndex: '1',
});
let toolbar: SubstituteToolbarLayout | null = null;

window.progressBrowserProof = {
  /** Mount or retire the toolbar row that owns the tracks' placement. */
  toolbar(present): void {
    if (present) {
      frame.after(line);
      line.after(popup);
      toolbar = new SubstituteToolbarLayout(document, frame, bars.element);
    } else {
      toolbar?.dispose();
      toolbar = null;
      line.remove();
      popup.remove();
    }
  },
  /** Keep the native expanded queue available independently of compact generation feedback. */
  queueExpanded(expanded): void {
    queue.setAttribute('aria-pressed', String(expanded));
  },
  /** Publish a finite projection through the same retained view used by the frame. */
  update(visible, workflow, sampler, dark): void {
    frame.dataset.colorScheme = dark ? 'dark' : 'light';
    bars.update({ visible, workflow, sampler });
  },
  /** Restore native presentation when the frame preference is disabled. */
  mode(enabled): void {
    document.documentElement.toggleAttribute('data-sc-substitute-frame', enabled);
    if (!enabled) bars.update({ visible: false, workflow: 0, sampler: 0 });
  },
};
