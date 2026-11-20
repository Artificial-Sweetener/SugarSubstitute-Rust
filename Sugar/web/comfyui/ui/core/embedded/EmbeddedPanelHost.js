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
/** Bind one browser document to negotiated host presentation and existing Comfy connectivity. */
import { DeclaredHost, HOST_CAPABILITIES, HOST_PROTOCOLS, PanelClientSession, isHostDescriptionMessage, } from './PanelRuntime.js';
import { createUxpPanelTransport } from './UxpPanelTransport.js';
import { createNativePanelTransport } from './NativePanelTransport.js';
import { EmbeddedPanelLayout } from './EmbeddedPanelLayout.js';
import { MemoryHostLink } from '../memory/MemoryHostLink.js';
import { UpdatesHostLink } from '../updates/UpdatesHostLink.js';
import { HostReleaseNotesLink } from '../release_notes/HostReleaseNotesLink.js';
import { HostSessionNoticeLink, } from '../session_notice/HostSessionNoticeLink.js';
import { HostBackendLink } from '../backend/HostBackendLink.js';
import { HostUpdateReportLink, } from '../update_report/HostUpdateReportLink.js';
import { HostFileDragLink } from '../file_drag/HostFileDragLink.js';
import { HostFileChoiceLink } from '../file_choice/HostFileChoiceLink.js';
/** Keep messaging active only while the owning SugarCubes document is alive. */
export class EmbeddedPanelHost {
    options;
    transport;
    #layout;
    #stop = null;
    #stopConnection = null;
    #memory = null;
    #updates = null;
    #releaseNotes = null;
    #sessionNotice = null;
    #fileDrag = null;
    #backend = null;
    #updateReport = null;
    #fileChoice = null;
    #stopHost = null;
    #timer = null;
    /** Receive an identified adapter transport after Comfy and presentation setup have succeeded. */
    constructor(options, transport) {
        this.options = options;
        this.transport = transport;
        this.#layout = new EmbeddedPanelLayout(options.window);
    }
    /** Listen before announcing readiness so synchronous host responses cannot be lost. */
    setup() {
        if (this.#stop)
            return;
        const client = new PanelClientSession(this.transport.session, {
            send: (message) => this.transport.send(message),
            apply: (presentation) => {
                const host = this.#declared();
                this.#layout.apply(presentation.resizeCornerSize);
                this.options.hostAccent?.set(presentation.accentColor ?? null);
                if (host.declares(HOST_CAPABILITIES.hostStyles))
                    this.options.presentation.set(presentation);
                if (host.serves(HOST_PROTOCOLS.canvas))
                    this.options.canvas?.connect(this.transport, this.options.window, this.options.report, {
                        masks: host.serves(HOST_PROTOCOLS.canvasMasks),
                    });
                return true;
            },
            connected: this.options.connected,
            report: this.options.report,
        });
        this.#stop = this.transport.subscribe((value) => {
            try {
                if (this.#describe(value))
                    return;
                client.receive(value);
            }
            catch (error) {
                this.options.report(error);
            }
        });
        let connected = this.options.connected();
        this.#stopConnection =
            this.options.subscribeConnection?.((value) => {
                if (connected === value)
                    return;
                connected = value;
                if (value)
                    this.options.canvas?.refresh();
                else
                    this.options.canvas?.disconnected();
            }) ?? null;
        const ready = () => {
            try {
                client.ready();
            }
            catch (error) {
                this.options.report(error);
            }
        };
        this.#stopHost = this.options.host.subscribe(() => this.#joinHostServices());
        this.#joinHostServices();
        this.#timer = this.options.window.setInterval(ready, 1000);
        this.options.window.addEventListener('pagehide', this.dispose);
        ready();
    }
    /**
     * Settle this session's host description; report one that is ambiguous or contradicts the first.
     *
     * @returns Whether the value was a host description for this session.
     */
    #describe(value) {
        if (!isHostDescriptionMessage(value) || value.session !== this.transport.session)
            return false;
        const declared = DeclaredHost.read(value);
        if (!declared) {
            this.options.report(new Error('The host described itself with a repeated protocol or capability.'));
            this.options.host.settle(DeclaredHost.undeclared);
        }
        else if (!this.options.host.settle(declared))
            this.options.report(new Error('The host changed its description during the panel session.'));
        return true;
    }
    /** A configure without a prior description settles on a host that serves only the panel. */
    #declared() {
        const settled = this.options.host.read();
        if (settled)
            return settled;
        this.options.host.settle(DeclaredHost.undeclared);
        return DeclaredHost.undeclared;
    }
    /** Join each optional host protocol once the settled description declares it. */
    #joinHostServices() {
        this.#joinMemoryBudget();
        this.#watchHostUpdates();
        this.#watchHostReleaseNotes();
        this.#watchHostSessionNotice();
        this.#followHostFileDrags();
        this.#watchHostBackend();
        this.#watchHostUpdateReport();
        this.#offerHostFileChoice();
    }
    /** Offer the host's file dialog to the page once the host declares that it shows one. */
    #offerHostFileChoice() {
        if (this.#fileChoice ||
            !this.options.fileChoice ||
            !this.options.host.read()?.serves(HOST_PROTOCOLS.fileChoice))
            return;
        this.#fileChoice = new HostFileChoiceLink(this.transport, this.options.report);
        this.#fileChoice.start();
        this.options.fileChoice.attach(this.#fileChoice);
    }
    /** Learn whether the host has an update report once the host declares that it says so. */
    #watchHostUpdateReport() {
        if (this.#updateReport ||
            !this.options.updateReport ||
            !this.options.host.read()?.serves(HOST_PROTOCOLS.updateReport))
            return;
        this.#updateReport = new HostUpdateReportLink(this.transport, this.options.updateReport, this.options.report);
        this.#updateReport.start();
    }
    /** Follow the ComfyUI the host runs once the host declares that it reports it. */
    #watchHostBackend() {
        if (this.#backend ||
            !this.options.backend ||
            !this.options.host.read()?.serves(HOST_PROTOCOLS.backend))
            return;
        this.#backend = new HostBackendLink(this.transport, this.options.backend, this.options.report);
        this.#backend.start();
    }
    /** Learn what a dragged file is called once the host declares that it says so. */
    #followHostFileDrags() {
        if (this.#fileDrag ||
            !this.options.draggedFiles ||
            !this.options.host.read()?.serves(HOST_PROTOCOLS.fileDrag))
            return;
        this.#fileDrag = new HostFileDragLink(this.transport, this.options.draggedFiles);
        this.#fileDrag.start();
    }
    /** Show the notice the host owes about the session once the host declares that it has them. */
    #watchHostSessionNotice() {
        const host = this.options.host.read();
        if (this.#sessionNotice ||
            !this.options.sessionNotice ||
            !host?.serves(HOST_PROTOCOLS.sessionNotice))
            return;
        try {
            this.#sessionNotice = new HostSessionNoticeLink(this.transport, this.options.sessionNotice, this.options.report, 
            // A host that also answers version two, whose notices include its own lines, says so.
            host.declares(HOST_CAPABILITIES.noticeLines) ? 2 : 1);
            this.#sessionNotice.start();
        }
        catch (error) {
            this.options.report(error);
        }
    }
    /** Show the host's release notes in What's new once the host declares that it serves them. */
    #watchHostReleaseNotes() {
        const host = this.options.host.read();
        if (this.#releaseNotes ||
            !this.options.releaseNotes ||
            !host?.serves(HOST_PROTOCOLS.releaseNotes))
            return;
        try {
            const link = new HostReleaseNotesLink(this.transport, this.options.report);
            this.#releaseNotes = { link, detach: this.options.releaseNotes.attachHost(link) };
            link.start();
        }
        catch (error) {
            this.options.report(error);
        }
    }
    /** Present the host's updater in the Updates entry once the host declares that it serves it. */
    #watchHostUpdates() {
        const host = this.options.host.read();
        if (this.#updates || !this.options.updates || !host)
            return;
        if (!host.serves(HOST_PROTOCOLS.updates))
            return;
        // A host that also answers version two, which names its product, says so with a capability.
        const version = host.declares(HOST_CAPABILITIES.namedUpdates) ? 2 : 1;
        try {
            const link = new UpdatesHostLink(this.transport, this.options.report, version);
            this.#updates = { link, detach: this.options.updates.attach(link) };
            link.start();
        }
        catch (error) {
            this.options.report(error);
        }
    }
    /** Join the desktop memory budget once the host declares that it serves it. */
    #joinMemoryBudget() {
        const host = this.options.host.read();
        if (this.#memory || !this.options.memory || !host?.serves(HOST_PROTOCOLS.memory))
            return;
        try {
            this.#memory = new MemoryHostLink(this.transport, this.options.memory.registry, this.options.memory.preferences(), this.options.report);
            this.#memory.start();
        }
        catch (error) {
            this.options.report(error);
        }
    }
    /** Retire subscriptions and restore saved preferences when the embedded document leaves. */
    dispose = () => {
        this.#stop?.();
        this.#stop = null;
        this.#stopConnection?.();
        this.#stopConnection = null;
        this.#stopHost?.();
        this.#stopHost = null;
        this.#memory?.dispose();
        this.#memory = null;
        this.#updates?.detach();
        this.#updates?.link.dispose();
        this.#updates = null;
        this.#releaseNotes?.detach();
        this.#releaseNotes?.link.dispose();
        this.#releaseNotes = null;
        this.#sessionNotice?.dispose();
        this.#sessionNotice = null;
        this.#fileDrag?.dispose();
        this.#fileDrag = null;
        this.#backend?.dispose();
        this.#backend = null;
        this.#updateReport?.dispose();
        this.#updateReport = null;
        if (this.#fileChoice)
            this.options.fileChoice?.attach(null);
        this.#fileChoice?.dispose();
        this.#fileChoice = null;
        if (this.#timer !== null)
            this.options.window.clearInterval(this.#timer);
        this.#timer = null;
        this.options.window.removeEventListener('pagehide', this.dispose);
        this.options.presentation.set(null);
        this.options.hostAccent?.set(null);
        this.options.canvas?.dispose();
        this.#layout.dispose();
    };
}
/**
 * Activate embedded presentation only for an identified application WebView.
 *
 * A page with no identified host settles on the standalone declaration at once.
 */
export function startEmbeddedPanel(options) {
    const transport = identifiedTransport(options.window);
    if (!transport) {
        options.host.settle(DeclaredHost.standalone);
        return null;
    }
    const host = new EmbeddedPanelHost(options, transport);
    host.setup();
    return host;
}
/** Find the one transport this navigation identifies, refusing an ambiguous transport choice. */
function identifiedTransport(windowRef) {
    const query = new URLSearchParams(windowRef.location.search);
    const kind = query.get('sc-panel-transport');
    if (query.getAll('sc-panel-transport').length > 1 || (kind !== null && kind !== 'native'))
        return null;
    const native = createNativePanelTransport(windowRef);
    return kind === 'native' ? native : (native ?? createUxpPanelTransport(windowRef));
}
