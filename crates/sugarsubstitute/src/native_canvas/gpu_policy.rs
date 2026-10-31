// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Choose the embedded canvas GPU from the enumerated adapters under one explicit policy.

use sugarsubstitute_contracts::configuration::CanvasRendererPreference;
use sugarsubstitute_image_gpu::{AdapterDescriptor, AdapterId, AdapterKey};

/// Renderer policy for the canvas embedded in the main window.
///
/// The embedded canvas never fails because of this policy: an unavailable adapter, device or
/// composition interop presents CPU-prepared frames instead.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum EmbeddedGpuPolicy {
    /// Prepare and present every frame on the CPU.
    #[default]
    Cpu,
    /// Render on the uniquely matching compatible adapter when it is present.
    Adapter(AdapterKey),
}

/// GPU renderer request carried by one canvas session; absence selects CPU presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GpuRequest {
    /// The standalone viewer's explicit selector; an absent or ambiguous adapter fails the session.
    Required(AdapterKey),
    /// The embedded canvas policy; an unavailable adapter falls back to CPU presentation.
    /// Only the main-window composition host constructs it.
    #[cfg_attr(not(all(windows, feature = "native-composition")), allow(dead_code))]
    Embedded(EmbeddedGpuPolicy),
}

impl EmbeddedGpuPolicy {
    /// Session request for this policy, or none when it always presents CPU frames.
    #[cfg_attr(not(all(windows, feature = "native-composition")), allow(dead_code))]
    pub(crate) const fn request(self) -> Option<GpuRequest> {
        match self {
            Self::Cpu => None,
            Self::Adapter(_) => Some(GpuRequest::Embedded(self)),
        }
    }
}

/// Interpret the stored renderer preference as the embedded canvas policy.
///
/// An absent preference or the stored text `cpu` selects CPU frames, and an adapter selector such
/// as `vulkan:10de:2b85` selects that adapter. Anything else is a miss whose reason the caller
/// logs before presenting CPU frames; the stored value is never rewritten.
/// # Errors
/// Returns the stable miss reason for malformed or unrecognized stored values.
pub(crate) fn embedded_policy(
    preference: &CanvasRendererPreference,
) -> Result<EmbeddedGpuPolicy, &'static str> {
    match preference {
        CanvasRendererPreference::Absent => Ok(EmbeddedGpuPolicy::Cpu),
        CanvasRendererPreference::Malformed => Err("malformed_renderer_preference"),
        CanvasRendererPreference::Stored(text) if text == "cpu" => Ok(EmbeddedGpuPolicy::Cpu),
        CanvasRendererPreference::Stored(text) => text
            .parse()
            .map(EmbeddedGpuPolicy::Adapter)
            .map_err(|_| "unrecognized_renderer_preference"),
    }
}

/// Select the embedded canvas adapter from one discovery result.
///
/// This is the only owner of the embedded selection rule. It never substitutes another adapter
/// for an absent, ambiguous or image-kernel-incompatible match.
pub(crate) fn select_embedded_gpu<'a>(
    adapters: impl Iterator<Item = &'a AdapterDescriptor>,
    policy: EmbeddedGpuPolicy,
) -> Option<AdapterId> {
    match policy {
        EmbeddedGpuPolicy::Cpu => None,
        EmbeddedGpuPolicy::Adapter(key) => {
            let mut matches = adapters.filter(|adapter| adapter.key() == key);
            let selected = matches.next()?;
            (matches.next().is_none() && selected.compatible).then_some(selected.id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_image_gpu::{AdapterClass, Backend};

    fn adapter(index: u8, vendor: u32, device: u32, compatible: bool) -> AdapterDescriptor {
        AdapterDescriptor {
            id: AdapterId::fixture(index),
            name: format!("Adapter {index}"),
            vendor,
            device,
            backend: Backend::Vulkan,
            class: AdapterClass::Discrete,
            compatible,
            luid: None,
            shared_textures: false,
        }
    }

    #[test]
    fn stored_preferences_select_cpu_an_adapter_or_an_observable_miss()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = |preference| embedded_policy(&preference);
        assert_eq!(policy(CanvasRendererPreference::Absent), Ok(EmbeddedGpuPolicy::Cpu));
        assert_eq!(policy(CanvasRendererPreference::stored("cpu")), Ok(EmbeddedGpuPolicy::Cpu));
        assert_eq!(
            policy(CanvasRendererPreference::stored("vulkan:10de:2b85")),
            Ok(EmbeddedGpuPolicy::Adapter("vulkan:10de:2b85".parse()?))
        );
        for text in ["dx12:10de:2b85", "vulkan:10de", "", "GPU"] {
            assert_eq!(
                policy(CanvasRendererPreference::stored(text)),
                Err("unrecognized_renderer_preference")
            );
        }
        assert_eq!(
            policy(CanvasRendererPreference::Malformed),
            Err("malformed_renderer_preference")
        );
        assert_eq!(
            policy(CanvasRendererPreference::stored(&"x".repeat(65))),
            Err("malformed_renderer_preference")
        );
        Ok(())
    }

    #[test]
    fn cpu_policy_never_selects_and_adapter_policy_selects_only_its_unique_compatible_match()
    -> Result<(), Box<dyn std::error::Error>> {
        let integrated = adapter(0, 0x8086, 0xa780, true);
        let discrete = adapter(1, 0x10de, 0x2b85, true);
        let adapters = [integrated.clone(), discrete.clone()];
        assert_eq!(select_embedded_gpu(adapters.iter(), EmbeddedGpuPolicy::Cpu), None);
        assert_eq!(EmbeddedGpuPolicy::default(), EmbeddedGpuPolicy::Cpu);
        assert_eq!(EmbeddedGpuPolicy::Cpu.request(), None);
        for selected in &adapters {
            let policy = EmbeddedGpuPolicy::Adapter(selected.key());
            assert_eq!(select_embedded_gpu(adapters.iter(), policy), Some(selected.id));
            assert_eq!(select_embedded_gpu(adapters.iter().rev(), policy), Some(selected.id));
            assert_eq!(policy.request(), Some(GpuRequest::Embedded(policy)));
        }
        let absent = EmbeddedGpuPolicy::Adapter("vulkan:1002:73bf".parse()?);
        assert_eq!(select_embedded_gpu(adapters.iter(), absent), None);
        let twins = [discrete.clone(), adapter(2, 0x10de, 0x2b85, true)];
        let policy = EmbeddedGpuPolicy::Adapter(discrete.key());
        assert_eq!(select_embedded_gpu(twins.iter(), policy), None);
        let incompatible = [adapter(3, 0x10de, 0x2b85, false)];
        assert_eq!(select_embedded_gpu(incompatible.iter(), policy), None);
        Ok(())
    }
}
