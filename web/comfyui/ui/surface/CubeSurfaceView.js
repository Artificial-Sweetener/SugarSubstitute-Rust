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
/** Compose one Cube face around host-rendered native node cards. */
import { resolveCubeFaceCardPresentation, } from './CubeFaceCardPolicy.js';
import { setCubeFaceNodeEnabled } from './CubeFaceCardStateController.js';
import { NativeNodeCardHost } from './NativeNodeCardHost.js';
import { CubePreviewRailView } from './CubePreviewRailView.js';
import { CubePreviewDividerController } from './CubePreviewDividerController.js';
import { layoutCubeSurfaceDom } from './CubeSurfaceDomLayout.js';
import { NativeCardGeometryObserver } from './NativeCardGeometryObserver.js';
import { CubePreviewFrameResizePolicy } from './CubePreviewFrameResizePolicy.js';
import { CubeFaceHeaderView } from './CubeFaceHeaderView.js';
/** Own DOM composition, actions, and responsive geometry for one Cube face. */
export class CubeSurfaceView {
    element;
    header;
    #state;
    #nodes;
    #graph;
    #onStateChange;
    #onMinimumHeightChange;
    #cardHost;
    #headerView;
    #content;
    #masonry;
    #previewDivider;
    #previewRail;
    #previewView;
    #geometryObserver;
    #previewDividerController;
    #previewFrameResize = new CubePreviewFrameResizePolicy();
    #cells = [];
    #visibleCards = [];
    #lastLayoutWidth = 1;
    /** Build a Cube view without assuming ownership of Comfy's renderer service. */
    constructor(options) {
        this.#state = options.state;
        this.#nodes = [...options.nodes];
        this.#graph = options.graph;
        this.#onStateChange = options.onStateChange;
        this.#onMinimumHeightChange = options.onMinimumHeightChange ?? null;
        this.#cardHost = new NativeNodeCardHost(options.renderer);
        this.element = options.document.createElement('div');
        this.element.className = 'sugarcubes-cube-face';
        this.element.setAttribute('aria-label', `${options.identity.instanceTitle} Cube contents`);
        this.#headerView = new CubeFaceHeaderView({
            document: options.document,
            identity: options.identity,
            metadata: options.metadata ?? {},
            chromeActions: options.chromeActions ?? null,
        });
        this.header = this.#headerView.element;
        this.#content = options.document.createElement('div');
        this.#content.className = 'sugarcubes-cube-face__content';
        this.#content.dataset.cubeContent = '';
        this.#masonry = options.document.createElement('div');
        this.#masonry.className = 'sugarcubes-cube-face__masonry';
        this.#masonry.dataset.cubeMasonry = '';
        this.#masonry.style.position = 'relative';
        this.#previewRail = options.document.createElement('aside');
        this.#previewRail.className = 'sugarcubes-cube-face__preview';
        this.#previewRail.dataset.cubePreviewRail = '';
        this.#previewRail.setAttribute('aria-label', 'Cube output preview');
        this.#previewView = new CubePreviewRailView(this.#previewRail, options.previewActions ?? null);
        this.#previewDivider = options.document.createElement('button');
        this.#previewDivider.type = 'button';
        this.#previewDivider.className = 'sugarcubes-cube-face__preview-divider';
        this.#previewDivider.dataset.cubePreviewDivider = '';
        this.#previewDivider.setAttribute('aria-label', 'Resize Cube output preview');
        const windowRef = options.document.defaultView;
        this.#previewDividerController = windowRef
            ? new CubePreviewDividerController({
                element: this.#previewDivider,
                events: windowRef,
                getScale: options.getScale ?? (() => 1),
                onResize: (width, committed) => {
                    this.#state.preview.width = width;
                    this.layout(this.#lastLayoutWidth);
                    if (committed)
                        this.#onStateChange(this.#state);
                },
            })
            : null;
        this.#content.append(this.#masonry, this.#previewDivider, this.#previewRail);
        this.element.append(this.header, this.#content);
        this.#geometryObserver = new NativeCardGeometryObserver(options.document, () => this.layout(this.#lastLayoutWidth));
        this.#renderCards();
    }
    /** Render current output and internal-node media into the dedicated rail. */
    renderPreview(snapshot) {
        this.#previewView.render(snapshot);
    }
    /** Reconcile live action eligibility without rebuilding the mounted Cube face. */
    renderHeaderActions() {
        this.#headerView.renderActions();
    }
    /** Reserve only the boundary gutters backed by real Cube ports. */
    setPortGutterWidths(inputWidth, outputWidth) {
        this.element.style.setProperty('--sugarcubes-cube-input-gutter-width', `${String(Math.max(0, inputWidth))}px`);
        this.element.style.setProperty('--sugarcubes-cube-output-gutter-width', `${String(Math.max(0, outputWidth))}px`);
    }
    /** Reflow while assigning ordinary outer width changes to the preview rail. */
    layout(width) {
        const safeWidth = Number.isFinite(width) ? Math.max(1, width) : 1;
        this.#lastLayoutWidth = safeWidth;
        let result = this.#applyLayout(safeWidth);
        const resizedPreviewWidth = this.#previewFrameResize.resolve({
            frameWidth: safeWidth,
            previewWidth: result.previewWidth,
            range: result.previewWidthRange,
            active: result.previewResizable,
        });
        if (Math.abs(resizedPreviewWidth - result.previewWidth) >= 0.5) {
            this.#state.preview.width = resizedPreviewWidth;
            result = this.#applyLayout(safeWidth);
            this.#onStateChange(this.#state);
        }
        if (result.minimumHeight !== null) {
            this.#onMinimumHeightChange?.(result.minimumHeight);
        }
        this.#previewDividerController?.setGeometry(result.previewWidth, result.previewWidthRange, result.previewResizable);
        this.#previewView.reflow();
    }
    /** Apply one DOM layout pass without owning allocation policy. */
    #applyLayout(width) {
        return layoutCubeSurfaceDom({
            width,
            state: this.#state,
            cards: this.#visibleCards,
            cells: this.#cells,
            content: this.#content,
            masonry: this.#masonry,
            previewDivider: this.#previewDivider,
            previewRail: this.#previewRail,
        });
    }
    /** Remove native card mounts owned by this view. */
    dispose() {
        this.#headerView.dispose();
        this.#previewDividerController?.dispose();
        this.#previewView.dispose();
        this.#geometryObserver.dispose();
        this.#cardHost.dispose();
    }
    /** Reconcile exact native card mounts from the shared presentation policy. */
    #renderCards() {
        const presentation = resolveCubeFaceCardPresentation(this.#nodes, this.#state, this.#graph);
        this.#visibleCards = presentation.cards.filter((card) => card.visible);
        this.#cells = this.#cardHost.mount(this.#masonry, this.#visibleCards, (node, enabled) => {
            setCubeFaceNodeEnabled(this.#state, node, enabled);
            this.#commitCardChange();
        });
        this.#geometryObserver.observe(this.#cells);
        this.#headerView.renderActions();
    }
    /** Persist one card change, then remount against the resulting policy. */
    #commitCardChange() {
        this.#onStateChange(this.#state);
        this.#renderCards();
        this.layout(this.#lastLayoutWidth);
    }
}
