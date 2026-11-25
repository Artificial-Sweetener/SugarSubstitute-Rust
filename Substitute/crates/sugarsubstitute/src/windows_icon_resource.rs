// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Turn the application's ICO file into the Windows resource that gives every executable its icon.
//!
//! The build script links the result into each binary of this package; Explorer, the taskbar and
//! shortcuts show the first icon group a program holds. The resource is written directly in the
//! documented `.res` layout, so no resource compiler is needed on the build machine.

/// Resource type of one icon image.
const RT_ICON: u16 = 3;
/// Resource type of the icon group that lists those images.
const RT_GROUP_ICON: u16 = 14;
/// The group's resource name, the one Windows shows for the program.
const GROUP_NAME: u16 = 1;

/// One image of an icon file, as its directory entry describes it.
struct IconImage<'a> {
    width: u8,
    height: u8,
    colors: u8,
    planes: u16,
    bit_count: u16,
    data: &'a [u8],
}

/// Build a `.res` file holding every image of `ico` and the group that names them.
pub(crate) fn icon_resource(ico: &[u8]) -> Result<Vec<u8>, String> {
    let images = read_icon(ico)?;
    let count = u16::try_from(images.len()).map_err(|_| "too many icon images".to_string())?;
    let mut resource = Vec::new();
    // A resource file starts with one empty entry that marks its 32-bit format.
    write_entry(&mut resource, 0, 0, &[], 0)?;
    let mut group = Vec::new();
    group.extend_from_slice(&0_u16.to_le_bytes());
    group.extend_from_slice(&1_u16.to_le_bytes());
    group.extend_from_slice(&count.to_le_bytes());
    for (id, image) in (1..=count).zip(&images) {
        write_entry(&mut resource, RT_ICON, id, image.data, 0x1010)?;
        group.extend_from_slice(&[image.width, image.height, image.colors, 0]);
        group.extend_from_slice(&image.planes.to_le_bytes());
        group.extend_from_slice(&image.bit_count.to_le_bytes());
        let size =
            u32::try_from(image.data.len()).map_err(|_| "icon image too large".to_string())?;
        group.extend_from_slice(&size.to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    write_entry(&mut resource, RT_GROUP_ICON, GROUP_NAME, &group, 0x1030)?;
    Ok(resource)
}

/// Read an ICO file's directory and the bytes of each image it lists.
fn read_icon(ico: &[u8]) -> Result<Vec<IconImage<'_>>, String> {
    let bytes = |at: usize, length: usize| -> Result<&[u8], String> {
        ico.get(at..at + length).ok_or_else(|| "icon file is truncated".to_string())
    };
    let word = |at: usize| -> Result<u16, String> {
        let value = bytes(at, 2)?;
        Ok(u16::from_le_bytes([value[0], value[1]]))
    };
    let dword = |at: usize| -> Result<usize, String> {
        let value = bytes(at, 4)?;
        usize::try_from(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
            .map_err(|error| error.to_string())
    };
    if word(0)? != 0 || word(2)? != 1 {
        return Err("not an icon file".into());
    }
    let count = usize::from(word(4)?);
    if count == 0 {
        return Err("icon file holds no images".into());
    }
    (0..count)
        .map(|index| {
            let entry = 6 + index * 16;
            let size = dword(entry + 8)?;
            let offset = dword(entry + 12)?;
            let head = bytes(entry, 4)?;
            Ok(IconImage {
                width: head[0],
                height: head[1],
                colors: head[2],
                planes: word(entry + 4)?,
                bit_count: word(entry + 6)?,
                data: bytes(offset, size)?,
            })
        })
        .collect()
}

/// Append one resource entry with an ordinal type and name, padded to a 32-bit boundary.
fn write_entry(
    resource: &mut Vec<u8>,
    kind: u16,
    name: u16,
    data: &[u8],
    flags: u16,
) -> Result<(), String> {
    let size = u32::try_from(data.len()).map_err(|_| "resource too large".to_string())?;
    resource.extend_from_slice(&size.to_le_bytes());
    resource.extend_from_slice(&32_u32.to_le_bytes());
    for ordinal in [kind, name] {
        resource.extend_from_slice(&0xFFFF_u16.to_le_bytes());
        resource.extend_from_slice(&ordinal.to_le_bytes());
    }
    resource.extend_from_slice(&0_u32.to_le_bytes());
    resource.extend_from_slice(&flags.to_le_bytes());
    resource.extend_from_slice(&0_u16.to_le_bytes());
    resource.extend_from_slice(&0_u32.to_le_bytes());
    resource.extend_from_slice(&0_u32.to_le_bytes());
    resource.extend_from_slice(data);
    while !resource.len().is_multiple_of(4) {
        resource.push(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::icon_resource;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The application's own icon file, as the build script reads it.
    const APP_ICON: &[u8] =
        include_bytes!("../../../../Sugar/integrations/contracts/ui/icons/app-icon.ico");

    /// Read back every resource entry: type, name and data.
    fn entries(resource: &[u8]) -> Result<Vec<(u16, u16, Vec<u8>)>, Box<dyn std::error::Error>> {
        let mut at = 0;
        let mut found = Vec::new();
        while at + 32 <= resource.len() {
            let size = usize::try_from(u32::from_le_bytes(resource[at..at + 4].try_into()?))?;
            let kind = u16::from_le_bytes([resource[at + 10], resource[at + 11]]);
            let name = u16::from_le_bytes([resource[at + 14], resource[at + 15]]);
            found.push((kind, name, resource[at + 32..at + 32 + size].to_vec()));
            at += 32 + size;
            at += (4 - at % 4) % 4;
        }
        Ok(found)
    }

    #[test]
    fn every_image_of_the_app_icon_is_a_resource_and_one_group_names_them_all() -> TestResult {
        let resource = icon_resource(APP_ICON)?;
        let entries = entries(&resource)?;
        let count = usize::from(u16::from_le_bytes([APP_ICON[4], APP_ICON[5]]));
        // The marker entry, every image, then the group.
        assert_eq!(entries.len(), count + 2);
        assert_eq!((entries[0].0, entries[0].1, entries[0].2.len()), (0, 0, 0));
        let images: Vec<_> = entries.iter().filter(|entry| entry.0 == 3).collect();
        assert_eq!(images.len(), count);
        let (kind, name, group) = entries.last().cloned().ok_or("no group")?;
        assert_eq!((kind, name), (14, 1));
        assert_eq!(usize::from(u16::from_le_bytes([group[4], group[5]])), count);
        // Each group entry names an image resource of exactly the size it states.
        for (index, image) in images.iter().enumerate() {
            let entry = &group[6 + index * 14..6 + (index + 1) * 14];
            let size = usize::try_from(u32::from_le_bytes(entry[8..12].try_into()?))?;
            assert_eq!(size, image.2.len());
            assert_eq!(u16::from_le_bytes([entry[12], entry[13]]), image.1);
            assert!(image.2.starts_with(b"\x89PNG"));
        }
        Ok(())
    }

    #[test]
    fn a_file_that_is_not_an_icon_is_refused() {
        assert!(icon_resource(b"\x89PNG not an icon").is_err());
        assert!(icon_resource(&[0, 0, 1, 0, 0, 0]).is_err());
    }
}
