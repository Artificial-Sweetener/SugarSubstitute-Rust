// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Owned inputs shared by asynchronous and synchronous construction drivers.

use super::*;

pub(super) struct Config {
  pub attrs: Option<WebViewAttributes<'static>>,
  pub platform: super::super::PlatformSpecificWebViewAttributes,
  pub data_directory: Option<PathBuf>,
  pub id: Option<String>,
  pub owned: bool,
  pub is_child: bool,
  pub bounds: Rect,
}

impl Config {
  pub fn new(
    attrs: WebViewAttributes<'_>,
    platform: super::super::PlatformSpecificWebViewAttributes,
    owned: bool,
    is_child: bool,
  ) -> Result<Self> {
    if owned {
      validate(&attrs, &platform)?;
    }
    let data_directory = attrs
      .context
      .as_deref()
      .and_then(|c| c.data_directory())
      .map(|path| {
        let mut owned = PathBuf::new();
        owned
          .try_reserve_exact(path.as_os_str().len())
          .map_err(|cause| Error::Io(std::io::Error::other(cause)))?;
        owned.push(path);
        Ok::<_, Error>(owned)
      })
      .transpose()?;
    let id = attrs.id.map(str::to_owned);
    let bounds = attrs.bounds.unwrap_or_default();
    let attrs = WebViewAttributes {
      id: None,
      context: None,
      user_agent: attrs.user_agent,
      visible: attrs.visible,
      transparent: attrs.transparent,
      background_color: attrs.background_color,
      url: attrs.url,
      headers: attrs.headers,
      zoom_hotkeys_enabled: attrs.zoom_hotkeys_enabled,
      html: attrs.html,
      initialization_scripts: attrs.initialization_scripts,
      custom_protocols: attrs.custom_protocols,
      ipc_handler: attrs.ipc_handler,
      drag_drop_handler: attrs.drag_drop_handler,
      navigation_handler: attrs.navigation_handler,
      download_started_handler: attrs.download_started_handler,
      download_completed_handler: attrs.download_completed_handler,
      new_window_req_handler: attrs.new_window_req_handler,
      clipboard: attrs.clipboard,
      devtools: attrs.devtools,
      accept_first_mouse: attrs.accept_first_mouse,
      back_forward_navigation_gestures: attrs.back_forward_navigation_gestures,
      document_title_changed_handler: attrs.document_title_changed_handler,
      incognito: attrs.incognito,
      autoplay: attrs.autoplay,
      on_page_load_handler: attrs.on_page_load_handler,
      proxy_config: attrs.proxy_config,
      focused: attrs.focused,
      bounds: attrs.bounds,
      background_throttling: attrs.background_throttling,
      javascript_disabled: attrs.javascript_disabled,
      permission_handler: attrs.permission_handler,
      general_autofill_enabled: attrs.general_autofill_enabled,
    };
    Ok(Self {
      attrs: Some(attrs),
      platform,
      data_directory,
      id,
      owned,
      is_child,
      bounds,
    })
  }
}

pub(super) fn invalid(message: &'static str) -> Error {
  Error::Io(std::io::Error::new(
    std::io::ErrorKind::InvalidInput,
    message,
  ))
}

fn validate(
  attrs: &WebViewAttributes<'_>,
  platform: &super::super::PlatformSpecificWebViewAttributes,
) -> Result<()> {
  if attrs.visible
    || attrs.focused
    || attrs.url.as_deref() != Some("about:blank")
    || attrs.html.is_some()
    || attrs.headers.is_some()
    || attrs.id.is_some()
    || !attrs.initialization_scripts.is_empty()
    || attrs.initialization_scripts.capacity() != 0
    || !attrs.custom_protocols.is_empty()
    || attrs.custom_protocols.capacity() != 0
    || attrs.ipc_handler.is_some()
    || attrs.drag_drop_handler.is_some()
    || attrs.navigation_handler.is_some()
    || attrs.new_window_req_handler.is_some()
    || attrs.download_started_handler.is_some()
    || attrs.download_completed_handler.is_some()
    || attrs.document_title_changed_handler.is_some()
    || attrs.on_page_load_handler.is_some()
    || attrs.permission_handler.is_some()
    || attrs.clipboard
    || attrs.proxy_config.is_some()
    || attrs.user_agent.is_some()
    || attrs.incognito
    || platform.environment.is_some()
    || platform.browser_extensions_enabled
    || platform.extension_path.is_some()
    || match platform.additional_browser_args.as_deref() {
      Some("") => false,
      Some("--disk-cache-size=33554432") if platform.exclusive_user_data_folder_access => false,
      _ => true,
    }
  {
    return Err(invalid("unsupported owned webview configuration"));
  }
  if attrs.url.as_ref().is_some_and(|s| s.capacity() > 64)
    || platform
      .additional_browser_args
      .as_ref()
      .is_some_and(|s| s.capacity() > 64)
  {
    return Err(invalid("owned webview input capacity exceeded"));
  }
  let path = attrs
    .context
    .as_deref()
    .and_then(|c| c.data_directory())
    .ok_or_else(|| invalid("owned webview requires an admitted profile path"))?;
  use std::os::windows::ffi::OsStrExt;
  if !path.is_absolute()
    || path.as_os_str().encode_wide().count() > 32767
    || path.as_os_str().encode_wide().any(|u| u == 0)
    || path.as_os_str().len() > 131072
  {
    return Err(invalid("invalid owned profile path"));
  }
  if platform
    .profile_name
    .as_ref()
    .is_some_and(|s| s.len() > 64 || s.capacity() > 64 || s.contains('\0'))
  {
    return Err(invalid("invalid owned profile name"));
  }
  Ok(())
}
