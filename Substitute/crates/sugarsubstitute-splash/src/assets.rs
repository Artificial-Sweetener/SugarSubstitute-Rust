// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bind the reviewed character inventory to compiled-in resources.

use crate::PoseAsset;

/// Preserve all numbered characters followed by the original named guests.
pub static POSE_ASSETS: &[PoseAsset] = &[
    PoseAsset { name: "1.png", bytes: include_bytes!("../assets/poses/1.png") },
    PoseAsset { name: "2.png", bytes: include_bytes!("../assets/poses/2.png") },
    PoseAsset { name: "3.png", bytes: include_bytes!("../assets/poses/3.png") },
    PoseAsset { name: "4.png", bytes: include_bytes!("../assets/poses/4.png") },
    PoseAsset { name: "5.png", bytes: include_bytes!("../assets/poses/5.png") },
    PoseAsset { name: "6.png", bytes: include_bytes!("../assets/poses/6.png") },
    PoseAsset { name: "7.png", bytes: include_bytes!("../assets/poses/7.png") },
    PoseAsset { name: "8.png", bytes: include_bytes!("../assets/poses/8.png") },
    PoseAsset { name: "9.png", bytes: include_bytes!("../assets/poses/9.png") },
    PoseAsset { name: "10.png", bytes: include_bytes!("../assets/poses/10.png") },
    PoseAsset { name: "11.png", bytes: include_bytes!("../assets/poses/11.png") },
    PoseAsset { name: "12.png", bytes: include_bytes!("../assets/poses/12.png") },
    PoseAsset { name: "13.png", bytes: include_bytes!("../assets/poses/13.png") },
    PoseAsset { name: "14.png", bytes: include_bytes!("../assets/poses/14.png") },
    PoseAsset { name: "15.png", bytes: include_bytes!("../assets/poses/15.png") },
    PoseAsset { name: "16.png", bytes: include_bytes!("../assets/poses/16.png") },
    PoseAsset { name: "17.png", bytes: include_bytes!("../assets/poses/17.png") },
    PoseAsset { name: "18.png", bytes: include_bytes!("../assets/poses/18.png") },
    PoseAsset { name: "19.png", bytes: include_bytes!("../assets/poses/19.png") },
    PoseAsset { name: "20.png", bytes: include_bytes!("../assets/poses/20.png") },
    PoseAsset { name: "21.png", bytes: include_bytes!("../assets/poses/21.png") },
    PoseAsset { name: "22.png", bytes: include_bytes!("../assets/poses/22.png") },
    PoseAsset { name: "23.png", bytes: include_bytes!("../assets/poses/23.png") },
    PoseAsset { name: "24.png", bytes: include_bytes!("../assets/poses/24.png") },
    PoseAsset { name: "25.png", bytes: include_bytes!("../assets/poses/25.png") },
    PoseAsset { name: "26.png", bytes: include_bytes!("../assets/poses/26.png") },
    PoseAsset { name: "27.png", bytes: include_bytes!("../assets/poses/27.png") },
    PoseAsset { name: "28.png", bytes: include_bytes!("../assets/poses/28.png") },
    PoseAsset { name: "cass.png", bytes: include_bytes!("../assets/poses/cass.png") },
    PoseAsset { name: "comfy.png", bytes: include_bytes!("../assets/poses/comfy.png") },
    PoseAsset { name: "comfy2.png", bytes: include_bytes!("../assets/poses/comfy2.png") },
    PoseAsset { name: "cubby.png", bytes: include_bytes!("../assets/poses/cubby.png") },
    PoseAsset { name: "diffusion.png", bytes: include_bytes!("../assets/poses/diffusion.png") },
    PoseAsset { name: "liz.png", bytes: include_bytes!("../assets/poses/liz.png") },
    PoseAsset { name: "lvgf.png", bytes: include_bytes!("../assets/poses/lvgf.png") },
    PoseAsset { name: "married.png", bytes: include_bytes!("../assets/poses/married.png") },
    PoseAsset {
        name: "pixelandlibrary.png",
        bytes: include_bytes!("../assets/poses/pixelandlibrary.png"),
    },
    PoseAsset { name: "ren.png", bytes: include_bytes!("../assets/poses/ren.png") },
    PoseAsset { name: "syl.png", bytes: include_bytes!("../assets/poses/syl.png") },
    PoseAsset { name: "witchy.png", bytes: include_bytes!("../assets/poses/witchy.png") },
];
