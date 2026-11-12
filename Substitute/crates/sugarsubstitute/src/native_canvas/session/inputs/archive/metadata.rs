// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode the host metadata of an input-document archive: version-one JSON recording every
//! workflow's surfaces with their document and layer identities, synced mask state and the
//! active input document.
//!
//! Decoding rejects unknown versions and fields, repeated or invalid identities and keys, and
//! layer identities outside their surface's allocation, so a restored mapping is always one the
//! canvas could have produced.
use super::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// The only metadata version this build writes and reads.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    version: u32,
    next_identity: u64,
    active: Option<Active>,
    workflows: Vec<WorkflowRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Active {
    document: u64,
    mask: Option<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowRecord {
    workflow: String,
    surfaces: Vec<SurfaceRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceRecord {
    key: String,
    label: String,
    document: u64,
    base: BaseRecord,
    loaded_base: Option<BaseRecord>,
    next_layer: u64,
    masks: Vec<MaskRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum BaseRecord {
    File { reference: String },
    Blank { width: u32, height: u32 },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaskRecord {
    key: String,
    label: String,
    layer: u64,
    color: [u8; 3],
    opacity: f32,
    channel: ChannelRecord,
    desired_initial: Option<String>,
    loaded_initial: Option<String>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ChannelRecord {
    Red,
    Green,
    Blue,
    Alpha,
}

/// Surfaces, identities and active state restored from metadata.
pub(super) struct RestoredMetadata {
    pub(super) next_identity: u64,
    pub(super) workflows: Vec<(Arc<str>, Vec<Surface>)>,
    pub(super) active: Option<(InputDocumentId, Option<LayerId>)>,
}

/// Encode `inputs`' synced state with the shown document and its active mask.
pub(super) fn encode(
    inputs: &InputDocuments,
    active: Option<(InputDocumentId, Option<LayerId>)>,
) -> Result<Vec<u8>, ArchiveError> {
    let metadata = Metadata {
        version: VERSION,
        next_identity: inputs.next_identity,
        active: active.map(|(document, mask)| Active {
            document: document.get() & !IDENTITY_NAMESPACE,
            mask: mask.map(LayerId::get),
        }),
        workflows: inputs
            .workflows
            .iter()
            .map(|(workflow, surfaces)| WorkflowRecord {
                workflow: workflow.to_string(),
                surfaces: surfaces.iter().map(surface_record).collect(),
            })
            .collect(),
    };
    serde_json::to_vec(&metadata).map_err(|_| ArchiveError::Invalid("Host metadata failed"))
}

/// Decode and validate metadata into surface records holding no documents.
pub(super) fn decode(bytes: &[u8]) -> Result<RestoredMetadata, Box<dyn Error>> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    if metadata.version != VERSION || metadata.next_identity & IDENTITY_NAMESPACE != 0 {
        return Err("Unsupported input-document host metadata".into());
    }
    let mut identities = Vec::new();
    let mut workflows: Vec<(Arc<str>, Vec<Surface>)> = Vec::new();
    for record in metadata.workflows {
        if workflows.iter().any(|(workflow, _)| **workflow == *record.workflow) {
            return Err("A workflow repeats in input-document metadata".into());
        }
        let mut surfaces: Vec<Surface> = Vec::new();
        for surface in record.surfaces {
            let surface = restore_surface(surface, metadata.next_identity)?;
            if identities.contains(&surface.identity)
                || surfaces.iter().any(|earlier| earlier.key == surface.key)
            {
                return Err("An input surface repeats in input-document metadata".into());
            }
            identities.push(surface.identity);
            surfaces.push(surface);
        }
        workflows.push((Arc::from(record.workflow), surfaces));
    }
    let active = metadata
        .active
        .map(|active| -> Result<_, Box<dyn Error>> {
            let document = identity(active.document, metadata.next_identity)?;
            Ok((document, active.mask.map(LayerId::new).transpose()?))
        })
        .transpose()?;
    Ok(RestoredMetadata { next_identity: metadata.next_identity, workflows, active })
}

/// Digest one surface's synced state for change detection, without allocating.
pub(super) fn hash_surface(surface: &Surface, hasher: &mut impl Hasher) {
    surface.key.hash(hasher);
    surface.label.hash(hasher);
    surface.identity.get().hash(hasher);
    hash_base(Some(&surface.desired_base), hasher);
    hash_base(surface.loaded_base.as_ref(), hasher);
    surface.next_layer.hash(hasher);
    surface.loaded.hash(hasher);
    surface.restored.is_some().hash(hasher);
    for mask in &surface.masks {
        mask.key.hash(hasher);
        mask.label.hash(hasher);
        mask.layer.get().hash(hasher);
        mask.appearance.color().hash(hasher);
        mask.appearance.opacity().to_bits().hash(hasher);
        (mask.channel as u8).hash(hasher);
        mask.desired_initial.hash(hasher);
        mask.loaded_initial.hash(hasher);
    }
}

fn hash_base(base: Option<&Base>, hasher: &mut impl Hasher) {
    match base {
        None => 0_u8.hash(hasher),
        Some(Base::File(reference)) => (1_u8, reference).hash(hasher),
        Some(Base::Blank(extent)) => (2_u8, extent.width(), extent.height()).hash(hasher),
    }
}
fn surface_record(surface: &Surface) -> SurfaceRecord {
    SurfaceRecord {
        key: surface.key.to_string(),
        label: surface.label.to_string(),
        document: surface.identity.get() & !IDENTITY_NAMESPACE,
        base: base_record(&surface.desired_base),
        loaded_base: surface.loaded_base.as_ref().map(base_record),
        next_layer: surface.next_layer,
        masks: surface
            .masks
            .iter()
            .map(|mask| MaskRecord {
                key: mask.key.to_string(),
                label: mask.label.to_string(),
                layer: mask.layer.get(),
                color: mask.appearance.color(),
                opacity: mask.appearance.opacity(),
                channel: match mask.channel {
                    MaskChannel::Red => ChannelRecord::Red,
                    MaskChannel::Green => ChannelRecord::Green,
                    MaskChannel::Blue => ChannelRecord::Blue,
                    MaskChannel::Alpha => ChannelRecord::Alpha,
                },
                desired_initial: mask.desired_initial.as_deref().map(str::to_owned),
                loaded_initial: mask.loaded_initial.as_deref().map(str::to_owned),
            })
            .collect(),
    }
}

fn base_record(base: &Base) -> BaseRecord {
    match base {
        Base::File(reference) => BaseRecord::File { reference: reference.to_string() },
        Base::Blank(extent) => BaseRecord::Blank { width: extent.width(), height: extent.height() },
    }
}

fn base(record: BaseRecord) -> Result<Base, Box<dyn Error>> {
    Ok(match record {
        BaseRecord::File { reference } => Base::File(Arc::from(reference)),
        BaseRecord::Blank { width, height } => Base::Blank(Extent::new(width, height)?),
    })
}

/// The input identity a recorded document number names, allocated below `next`.
fn identity(document: u64, next: u64) -> Result<InputDocumentId, Box<dyn Error>> {
    if document == 0 || document > next {
        return Err("An input document identity is outside its allocation".into());
    }
    Ok(InputDocumentId(IDENTITY_NAMESPACE | document))
}

fn restore_surface(record: SurfaceRecord, next: u64) -> Result<Surface, Box<dyn Error>> {
    let mut masks: Vec<Mask> = Vec::with_capacity(record.masks.len());
    if record.masks.len() > MAX_MASKS || record.next_layer <= BASE_LAYER {
        return Err("An input surface's masks are outside their bounds".into());
    }
    for mask in record.masks {
        if mask.layer <= BASE_LAYER
            || mask.layer >= record.next_layer
            || masks
                .iter()
                .any(|earlier| earlier.layer.get() == mask.layer || *earlier.key == *mask.key)
        {
            return Err("An input mask identity is outside its allocation".into());
        }
        masks.push(Mask {
            key: Arc::from(mask.key),
            label: Arc::from(mask.label),
            layer: LayerId::new(mask.layer)?,
            appearance: MaskAppearance::new(mask.color, mask.opacity)?,
            appearance_pending: false,
            channel: match mask.channel {
                ChannelRecord::Red => MaskChannel::Red,
                ChannelRecord::Green => MaskChannel::Green,
                ChannelRecord::Blue => MaskChannel::Blue,
                ChannelRecord::Alpha => MaskChannel::Alpha,
            },
            desired_initial: mask.desired_initial.map(Arc::from),
            loaded_initial: mask.loaded_initial.map(Arc::from),
            outcome: None,
        });
    }
    Ok(Surface {
        key: Arc::from(record.key),
        label: Arc::from(record.label),
        identity: identity(record.document, next)?,
        desired_base: base(record.base)?,
        loaded_base: None,
        masks,
        next_layer: record.next_layer,
        document: None,
        restored: None,
        loaded: false,
        failure: None,
    })
}
