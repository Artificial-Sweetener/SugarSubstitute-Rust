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
 * Own placement DOM overlay and event-listener lifecycle.
 */
/** Maintain placement capture UI and its host listeners. */
export class PlacementDomLifecycle {
    options;
    adapter;
    scheduler;
    state;
    pointer;
    constructor(options) {
        this.options = options;
        this.adapter = options.adapter;
        this.scheduler = options.scheduler;
        this.state = options.state;
        this.pointer = options.pointer;
    }
    updatePlacementOverlayBounds() {
        const overlay = this.state.overlayElement;
        const canvasElement = this.state.canvasElement;
        if (!overlay || !canvasElement || typeof canvasElement.getBoundingClientRect !== 'function') {
            return;
        }
        const rect = canvasElement.getBoundingClientRect();
        overlay.style.left = `${rect.left}px`;
        overlay.style.top = `${rect.top}px`;
        overlay.style.width = `${rect.width}px`;
        overlay.style.height = `${rect.height}px`;
    }
    stopPlacementOverlayLoop() {
        if (this.state.overlayRafId != null) {
            this.scheduler?.cancelRaf?.(this.state.overlayRafId);
            this.state.overlayRafId = null;
        }
    }
    startPlacementOverlayLoop() {
        this.stopPlacementOverlayLoop();
        const tick = () => {
            if (!this.state.active || !this.state.overlayElement) {
                this.state.overlayRafId = null;
                return;
            }
            this.updatePlacementOverlayBounds();
            this.state.overlayRafId = this.scheduler?.raf?.(tick);
        };
        this.state.overlayRafId = this.scheduler?.raf?.(tick);
    }
    ensurePlacementOverlay() {
        if (this.state.overlayElement) {
            return this.state.overlayElement;
        }
        const documentRef = this.adapter?.getDocument?.();
        if (!documentRef?.body) {
            return null;
        }
        const overlay = documentRef.createElement('div');
        overlay.className = 'sugarcubes-placement-overlay';
        Object.assign(overlay.style, {
            position: 'fixed',
            zIndex: '9005',
            background: 'transparent',
            pointerEvents: 'none',
            left: '0px',
            top: '0px',
            width: '0px',
            height: '0px',
        });
        documentRef.body.appendChild(overlay);
        this.state.overlayElement = overlay;
        return overlay;
    }
    showPlacementOverlay() {
        const overlay = this.ensurePlacementOverlay();
        if (!overlay) {
            return;
        }
        overlay.style.pointerEvents = 'auto';
        overlay.style.display = 'block';
        this.updatePlacementOverlayBounds();
        this.startPlacementOverlayLoop();
    }
    hidePlacementOverlay() {
        const overlay = this.state.overlayElement;
        if (!overlay) {
            return;
        }
        overlay.style.pointerEvents = 'none';
        overlay.style.display = 'none';
        this.stopPlacementOverlayLoop();
    }
    removePlacementHandlers() {
        if (!this.state.handlers) {
            return;
        }
        const { canvasElement, onPointerMove, onPointerDown, onMouseDown, onContextMenu, onKeyDown, overlayElement, onOverlayPointerMove, onOverlayPointerDown, onOverlayPointerUp, onOverlayContextMenu, } = this.state.handlers;
        if (canvasElement) {
            canvasElement.removeEventListener('pointermove', onPointerMove);
            canvasElement.removeEventListener('pointerdown', onPointerDown);
            canvasElement.removeEventListener('mousedown', onMouseDown);
            canvasElement.removeEventListener('contextmenu', onContextMenu);
        }
        if (overlayElement) {
            overlayElement.removeEventListener('pointermove', onOverlayPointerMove);
            overlayElement.removeEventListener('pointerdown', onOverlayPointerDown);
            overlayElement.removeEventListener('pointerup', onOverlayPointerUp);
            overlayElement.removeEventListener('contextmenu', onOverlayContextMenu);
        }
        this.adapter?.getWindow?.()?.removeEventListener?.('keydown', onKeyDown);
        this.state.handlers = null;
        this.state.canvasElement = null;
    }
    stop(_reason = null) {
        if (!this.state.active) {
            return;
        }
        this.removePlacementHandlers();
        this.hidePlacementOverlay();
        this.state.active = false;
        this.state.cubeId = null;
        this.state.defaultAlias = null;
        this.state.payload = null;
        this.state.baseOrigin = [0, 0];
        this.state.origin = [0, 0];
        this.state.commitInProgress = false;
        this.state.cubeVersion = '';
        this.state.cubeRevisionRef = 'WORKTREE';
        this.setPlacementSidebarVisibility(false);
        this.options.setDirty();
    }
    setPlacementSidebarVisibility(isActive) {
        const documentRef = this.adapter?.getDocument?.();
        if (!documentRef?.body) {
            return;
        }
        const active = Boolean(isActive);
        documentRef.body.classList.toggle('sugarcubes-placement--active', active);
        const panel = this.adapter?.getDocument?.()?.querySelector('.side-bar-panel') ||
            documentRef.querySelector('.side-bar-panel');
        if (!panel) {
            return;
        }
        if (active) {
            if (panel.dataset.sugarcubesDisplay === undefined) {
                panel.dataset.sugarcubesDisplay = panel.style.display || '';
            }
            panel.style.display = 'none';
            return;
        }
        if (panel.dataset.sugarcubesDisplay !== undefined) {
            panel.style.display = panel.dataset.sugarcubesDisplay;
            delete panel.dataset.sugarcubesDisplay;
        }
        else {
            panel.style.display = '';
        }
    }
    installPlacementHandlers() {
        const canvasInstance = this.pointer.getCanvas();
        const canvasElement = canvasInstance?.canvas ?? null;
        if (!canvasInstance || !canvasElement) {
            return false;
        }
        const overlayElement = this.ensurePlacementOverlay();
        if (!overlayElement) {
            return false;
        }
        const onPointerMove = (event) => {
            if (!this.state.active) {
                return;
            }
            const origin = this.pointer.resolvePlacementOrigin(canvasInstance, event);
            if (!origin) {
                return;
            }
            this.state.origin = origin;
            this.options.setDirty();
        };
        const onPointerDown = (event) => {
            this.pointer.handlePlacementMouseDown(event, canvasInstance, overlayElement);
        };
        const onMouseDown = (event) => {
            this.pointer.handlePlacementMouseDown(event, canvasInstance, overlayElement);
        };
        const onContextMenu = (event) => {
            if (!this.state.active) {
                return;
            }
            event.preventDefault();
        };
        const onOverlayPointerMove = (event) => {
            if (!this.state.active) {
                return;
            }
            const origin = this.pointer.resolvePlacementOrigin(canvasInstance, event);
            if (!origin) {
                return;
            }
            this.state.origin = origin;
            this.options.setDirty();
        };
        const onOverlayPointerDown = (event) => {
            this.pointer.handlePlacementMouseDown(event, canvasInstance, overlayElement);
        };
        const onOverlayPointerUp = (event) => {
            if (!this.state.active) {
                return;
            }
            event?.preventDefault?.();
            event?.stopPropagation?.();
            event?.stopImmediatePropagation?.();
            if (event) {
                event.cancelBubble = true;
            }
            if (overlayElement?.releasePointerCapture && event?.pointerId != null) {
                try {
                    overlayElement.releasePointerCapture(event.pointerId);
                }
                catch (_error) {
                    // ignore pointer capture failures
                }
            }
        };
        const onOverlayContextMenu = (event) => {
            if (!this.state.active) {
                return;
            }
            event.preventDefault();
        };
        const onKeyDown = (event) => {
            if (!this.state.active) {
                return;
            }
            if (event.key === 'Escape') {
                event.preventDefault();
                this.options.stop('Placement cancelled.');
            }
        };
        canvasElement.addEventListener('pointermove', onPointerMove);
        canvasElement.addEventListener('pointerdown', onPointerDown);
        canvasElement.addEventListener('mousedown', onMouseDown);
        canvasElement.addEventListener('contextmenu', onContextMenu);
        overlayElement.addEventListener('pointermove', onOverlayPointerMove);
        overlayElement.addEventListener('pointerdown', onOverlayPointerDown);
        overlayElement.addEventListener('pointerup', onOverlayPointerUp);
        overlayElement.addEventListener('contextmenu', onOverlayContextMenu);
        this.adapter?.getWindow?.()?.addEventListener?.('keydown', onKeyDown);
        this.state.handlers = {
            canvasElement,
            onPointerMove,
            onPointerDown,
            onMouseDown,
            onContextMenu,
            onKeyDown,
            overlayElement,
            onOverlayPointerMove,
            onOverlayPointerDown,
            onOverlayPointerUp,
            onOverlayContextMenu,
        };
        this.state.canvasElement = canvasElement;
        this.showPlacementOverlay();
        return true;
    }
}
