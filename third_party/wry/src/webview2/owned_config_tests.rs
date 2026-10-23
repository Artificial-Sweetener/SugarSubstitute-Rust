// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Owned configuration rejection has no native construction effect.

use super::*;

fn attrs(context: &mut crate::WebContext) -> WebViewAttributes<'_> {
  WebViewAttributes {
    context: Some(context),
    visible: false,
    focused: false,
    url: Some("about:blank".into()),
    download_started_handler: None,
    ..WebViewAttributes::default()
  }
}
fn platform() -> super::super::PlatformSpecificWebViewAttributes {
  super::super::PlatformSpecificWebViewAttributes {
    additional_browser_args: Some(String::new()),
    ..Default::default()
  }
}
fn context() -> crate::WebContext {
  crate::WebContext::new(Some(PathBuf::from(r"E:\synthetic-profile")))
}

#[test]
fn owned_configuration_detaches_profile_without_retaining_context_borrow() -> Result<()> {
  let mut context = context();
  let config = owned_config::Config::new(attrs(&mut context), platform(), true, true)?;
  assert!(config
    .attrs
    .as_ref()
    .is_some_and(|attrs| attrs.context.is_none()));
  assert_eq!(config.data_directory.as_deref(), context.data_directory());
  assert!(config
    .data_directory
    .as_ref()
    .is_some_and(|path| path.capacity() <= 131072));
  Ok(())
}

#[test]
fn owned_short_buffers_and_empty_containers_with_excess_capacity_are_rejected() {
  for variant in 0..4 {
    let mut context = context();
    let mut attrs = attrs(&mut context);
    let mut platform = platform();
    match variant {
      0 => {
        let mut text = String::with_capacity(1024);
        text.push_str("about:blank");
        attrs.url = Some(text);
      }
      1 => platform.additional_browser_args = Some(String::with_capacity(1024)),
      2 => attrs.initialization_scripts = Vec::with_capacity(16),
      _ => attrs.custom_protocols.reserve(16),
    }
    assert!(owned_config::Config::new(attrs, platform, true, true).is_err());
  }
}

#[test]
fn owned_queue_producers_and_generic_handlers_are_rejected_before_creation() {
  for variant in 0..5 {
    let mut context = context();
    let mut attrs = attrs(&mut context);
    match variant {
      0 => attrs.download_started_handler = Some(Box::new(|_, _| true)),
      1 => attrs.ipc_handler = Some(Box::new(|_| {})),
      2 => attrs.new_window_req_handler = Some(Box::new(|_, _| crate::NewWindowResponse::Deny)),
      3 => attrs.user_agent = Some("small".into()),
      _ => attrs.visible = true,
    }
    assert!(owned_config::Config::new(attrs, platform(), true, true).is_err());
  }
}

#[test]
fn owned_private_profile_admits_only_the_reviewed_cache_limit_argument() -> Result<()> {
  let mut context = context();
  let mut private = platform();
  private.exclusive_user_data_folder_access = true;
  private.additional_browser_args = Some("--disk-cache-size=33554432".into());
  assert!(owned_config::Config::new(attrs(&mut context), private, true, true).is_ok());
  for (exclusive, arguments) in [(false, "--disk-cache-size=33554432"),
    (true, "--disk-cache-size=33554433"), (true, "--disable-web-security"),
    (true, "--disk-cache-size=33554432 --disable-web-security")] {
    let mut private = platform();
    private.exclusive_user_data_folder_access = exclusive;
    private.additional_browser_args = Some(arguments.into());
    assert!(owned_config::Config::new(attrs(&mut context), private, true, true).is_err());
  }
  Ok(())
}
