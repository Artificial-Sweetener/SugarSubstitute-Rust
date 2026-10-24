import { NativeWidgetPresentation } from '../../surface/NativeWidgetPresentation.js';
import { NativeNodeMediaReplacement } from '../../surface/NativeNodeMediaPresentation.js';
const widgets = new WeakMap();
/** Share one widget visibility owner across simultaneous projections of the same exact loader. */
export class InputLoaderPresentation {
    node;
    fields;
    #widgets;
    #media;
    #disposed = false;
    /** Replace filename/upload presentation while retaining channel and all other loader settings. */
    constructor(node, root, fields) {
        this.node = node;
        this.fields = fields;
        let owner = widgets.get(node);
        if (!owner) {
            owner = { lease: new NativeWidgetPresentation('replacement'), references: 0 };
            widgets.set(node, owner);
        }
        owner.references++;
        this.#widgets = owner;
        this.#media = new NativeNodeMediaReplacement(root);
        this.refresh();
    }
    /** Reconcile native widgets introduced by host renderer or uploader lifecycles. */
    refresh() {
        return this.#widgets.lease.reconcile(new Set((this.node.widgets ?? []).filter((widget) => this.fields.includes(widget.name ?? '') ||
            widget.name === 'upload' ||
            widget.type === 'image_upload' ||
            (['SimpleSyrup.LoadMaskBatch', 'SimpleSyrup.LoadImageList'].includes(this.node.type ?? this.node.class_type ?? '') &&
                (widget.name === 'simple_syrup_replace_media' ||
                    widget.name === 'simple_syrup_add_media')))));
    }
    /** Restore graph-owned controls after the last replacement projection retires. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#media.dispose();
        this.#widgets.references--;
        if (!this.#widgets.references) {
            this.#widgets.lease.dispose();
            widgets.delete(this.node);
        }
    }
}
