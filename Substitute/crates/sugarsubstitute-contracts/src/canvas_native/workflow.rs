// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode `workflow.request`: SugarCubes' provisioning of workflow input surfaces and masks.
//!
//! Keys are SugarCubes-owned and stable across syncs and restarts; the host maps each key to
//! its own document and layer identities and must keep that mapping stable too.

use super::{
    CanvasColor, CanvasDimensions, CanvasLabel, CanvasMaskKey, CanvasNativeEnvelope,
    CanvasNativeError, CanvasOpacity, CanvasSurfaceKey, CanvasWorkflowKey,
    MAX_CANVAS_WORKFLOW_REQUEST_BYTES, admit, parse, wire,
};
use crate::canvas_surface::CanvasFileReference;

/// Which route the workflow focus shows.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasWorkflowRoute {
    /// The input documents.
    Input,
    /// Generated output.
    Output,
}

/// The image channel a mask loader reads; the host's coverage encoding serves every channel.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasMaskChannel {
    /// Red.
    Red,
    /// Green.
    Green,
    /// Blue.
    Blue,
    /// Alpha.
    Alpha,
}

/// Where a surface's base image comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanvasWorkflowSurfaceBase {
    /// A ComfyUI file; the Comfy adapter authorizes and acquires it.
    File(CanvasFileReference),
    /// A blank document of this extent, applied only when the surface is created.
    Blank(CanvasDimensions),
}

/// One mask of a workflow surface.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasWorkflowMask {
    /// SugarCubes' stable key.
    pub key: CanvasMaskKey,
    /// User-visible label.
    pub label: CanvasLabel,
    /// Overlay colour; applied only when it differs from the last synced value.
    pub color: CanvasColor,
    /// Overlay opacity; applied only when it differs from the last synced value.
    pub opacity: CanvasOpacity,
    /// Coverage to load when the mask is created or this reference changes; none never clears.
    pub initial: Option<CanvasFileReference>,
    /// The channel the mask's loader reads.
    pub channel: CanvasMaskChannel,
}

/// One workflow input surface and its masks.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasWorkflowSurface {
    /// SugarCubes' stable key.
    pub key: CanvasSurfaceKey,
    /// User-visible label.
    pub label: CanvasLabel,
    /// The base image source.
    pub base: CanvasWorkflowSurfaceBase,
    /// Up to 32 masks.
    pub masks: Vec<CanvasWorkflowMask>,
}

/// One mask's overlay appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasWorkflowAppearance {
    /// The surface holding the mask.
    pub surface: CanvasSurfaceKey,
    /// The mask.
    pub mask: CanvasMaskKey,
    /// Overlay colour.
    pub color: CanvasColor,
    /// Overlay opacity.
    pub opacity: CanvasOpacity,
}

/// Where existing content sits after a canvas-size change.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasResizeAnchor {
    /// Top-left corner.
    TopLeft,
    /// Top edge, centred.
    Top,
    /// Top-right corner.
    TopRight,
    /// Left edge, centred.
    Left,
    /// Centre.
    Center,
    /// Right edge, centred.
    Right,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, centred.
    Bottom,
    /// Bottom-right corner.
    BottomRight,
}

/// How scaling resamples content.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasResampling {
    /// Nearest neighbour.
    Nearest,
    /// Smooth filtering.
    Smooth,
}

/// How a resize treats existing content.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasWorkflowResizeMode {
    /// Change the canvas extent, keeping content at the anchor.
    Canvas(CanvasResizeAnchor),
    /// Scale content to the new extent.
    Scale(CanvasResampling),
}

/// One `workflow.request` command.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasWorkflowCommand {
    /// Replace the workflow's surface set; unlisted surfaces are retired.
    Sync {
        /// The workflow.
        workflow: CanvasWorkflowKey,
        /// Up to 64 surfaces.
        surfaces: Vec<CanvasWorkflowSurface>,
    },
    /// Show a surface and mask on a route.
    Activate {
        /// The workflow.
        workflow: CanvasWorkflowKey,
        /// The route to show.
        route: CanvasWorkflowRoute,
        /// The surface, if any.
        surface: Option<CanvasSurfaceKey>,
        /// The active mask, if any.
        mask: Option<CanvasMaskKey>,
    },
    /// Change mask overlay appearance as one undoable step.
    Appearance {
        /// The workflow.
        workflow: CanvasWorkflowKey,
        /// One to 2048 mask appearances.
        masks: Vec<CanvasWorkflowAppearance>,
    },
    /// Resize one surface; the reply is `synced` for that surface.
    Resize {
        /// The workflow.
        workflow: CanvasWorkflowKey,
        /// The surface.
        surface: CanvasSurfaceKey,
        /// The new extent.
        size: CanvasDimensions,
        /// How content is treated.
        mode: CanvasWorkflowResizeMode,
    },
    /// Highlight a hovered mask, or none.
    Hover {
        /// The workflow.
        workflow: CanvasWorkflowKey,
        /// The surface.
        surface: CanvasSurfaceKey,
        /// The hovered mask, if any.
        mask: Option<CanvasMaskKey>,
    },
}

/// A validated `workflow.request` from one panel session.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasWorkflowRequest {
    envelope: CanvasNativeEnvelope,
    command: CanvasWorkflowCommand,
}

impl CanvasWorkflowRequest {
    /// Decode the exact shared `workflow.request`.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, duplicate or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasNativeError> {
        admit(bytes, MAX_CANVAS_WORKFLOW_REQUEST_BYTES)?;
        let request: wire::CanvasWorkflowRequest = parse(bytes)?;
        if !request.valid() {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self {
            command: command(request.command)?,
            envelope: CanvasNativeEnvelope::new(request.session, request.request_id)?,
        })
    }

    /// Borrow the request's public correlation.
    pub const fn envelope(&self) -> &CanvasNativeEnvelope {
        &self.envelope
    }

    /// Borrow the decoded command.
    pub const fn command(&self) -> &CanvasWorkflowCommand {
        &self.command
    }
}

fn command(value: wire::CanvasWorkflowCommand) -> Result<CanvasWorkflowCommand, CanvasNativeError> {
    use wire::CanvasWorkflowCommand as Wire;
    Ok(match value {
        Wire::Sync { workflow, surfaces } => CanvasWorkflowCommand::Sync {
            workflow: CanvasWorkflowKey::new(workflow)?,
            surfaces: surfaces.into_iter().map(surface).collect::<Result<_, _>>()?,
        },
        Wire::Activate { workflow, route, surface, mask } => CanvasWorkflowCommand::Activate {
            workflow: CanvasWorkflowKey::new(workflow)?,
            route: match route.as_str() {
                "input" => CanvasWorkflowRoute::Input,
                "output" => CanvasWorkflowRoute::Output,
                _ => return Err(CanvasNativeError::Invalid),
            },
            surface: surface.map(CanvasSurfaceKey::new).transpose()?,
            mask: mask.map(CanvasMaskKey::new).transpose()?,
        },
        Wire::Appearance { workflow, masks } => CanvasWorkflowCommand::Appearance {
            workflow: CanvasWorkflowKey::new(workflow)?,
            masks: masks
                .into_iter()
                .map(|value| {
                    Ok(CanvasWorkflowAppearance {
                        surface: CanvasSurfaceKey::new(value.surface)?,
                        mask: CanvasMaskKey::new(value.mask)?,
                        color: CanvasColor::parse(&value.color)?,
                        opacity: CanvasOpacity::new(value.opacity)?,
                    })
                })
                .collect::<Result<_, CanvasNativeError>>()?,
        },
        Wire::Resize { workflow, surface, width, height, mode } => CanvasWorkflowCommand::Resize {
            workflow: CanvasWorkflowKey::new(workflow)?,
            surface: CanvasSurfaceKey::new(surface)?,
            size: CanvasDimensions::from_wire(width, height)?,
            mode: resize_mode(&mode)?,
        },
        Wire::Hover { workflow, surface, mask } => CanvasWorkflowCommand::Hover {
            workflow: CanvasWorkflowKey::new(workflow)?,
            surface: CanvasSurfaceKey::new(surface)?,
            mask: mask.map(CanvasMaskKey::new).transpose()?,
        },
    })
}

fn surface(value: wire::CanvasWorkflowSurface) -> Result<CanvasWorkflowSurface, CanvasNativeError> {
    Ok(CanvasWorkflowSurface {
        key: CanvasSurfaceKey::new(value.key)?,
        label: CanvasLabel::new(value.label)?,
        base: match value.base {
            wire::CanvasWorkflowSurfaceBase::File { file } => {
                CanvasWorkflowSurfaceBase::File(file_reference(file)?)
            }
            wire::CanvasWorkflowSurfaceBase::Blank { width, height } => {
                CanvasWorkflowSurfaceBase::Blank(CanvasDimensions::from_wire(width, height)?)
            }
        },
        masks: value.masks.into_iter().map(mask).collect::<Result<_, _>>()?,
    })
}

fn mask(value: wire::CanvasWorkflowMask) -> Result<CanvasWorkflowMask, CanvasNativeError> {
    Ok(CanvasWorkflowMask {
        key: CanvasMaskKey::new(value.key)?,
        label: CanvasLabel::new(value.label)?,
        color: CanvasColor::parse(&value.color)?,
        opacity: CanvasOpacity::new(value.opacity)?,
        initial: value.initial.map(file_reference).transpose()?,
        channel: match value.channel.as_str() {
            "red" => CanvasMaskChannel::Red,
            "green" => CanvasMaskChannel::Green,
            "blue" => CanvasMaskChannel::Blue,
            "alpha" => CanvasMaskChannel::Alpha,
            _ => return Err(CanvasNativeError::Invalid),
        },
    })
}

fn file_reference(
    value: wire::CanvasSurfaceFileReference,
) -> Result<CanvasFileReference, CanvasNativeError> {
    CanvasFileReference::from_parts(value.filename, value.subfolder, &value.r#type)
        .ok_or(CanvasNativeError::Invalid)
}

fn resize_mode(
    value: &wire::CanvasWorkflowResizeMode,
) -> Result<CanvasWorkflowResizeMode, CanvasNativeError> {
    Ok(match value {
        wire::CanvasWorkflowResizeMode::Canvas { anchor } => {
            CanvasWorkflowResizeMode::Canvas(match anchor.as_str() {
                "top-left" => CanvasResizeAnchor::TopLeft,
                "top" => CanvasResizeAnchor::Top,
                "top-right" => CanvasResizeAnchor::TopRight,
                "left" => CanvasResizeAnchor::Left,
                "center" => CanvasResizeAnchor::Center,
                "right" => CanvasResizeAnchor::Right,
                "bottom-left" => CanvasResizeAnchor::BottomLeft,
                "bottom" => CanvasResizeAnchor::Bottom,
                "bottom-right" => CanvasResizeAnchor::BottomRight,
                _ => return Err(CanvasNativeError::Invalid),
            })
        }
        wire::CanvasWorkflowResizeMode::Scale { resampling } => {
            CanvasWorkflowResizeMode::Scale(match resampling.as_str() {
                "nearest" => CanvasResampling::Nearest,
                "smooth" => CanvasResampling::Smooth,
                _ => return Err(CanvasNativeError::Invalid),
            })
        }
    })
}
