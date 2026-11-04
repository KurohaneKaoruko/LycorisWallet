//! USB removable-drive discovery for the cold-wallet feature.
//!
//! Cold wallet mode moves the encrypted vault file onto a removable drive so
//! the key material physically leaves the machine. This module answers two
//! questions: which removable drives are plugged in right now, and whether a
//! previously chosen drive is still mounted.
//!
//! Only the actual volume enumeration is platform specific. Everything else in
//! the cold-wallet flow works on plain paths and stays portable.

/// Marker file the cold-wallet flow writes next to the vault so the UI can
/// recognise "this drive is a Lycoris cold wallet" later.
pub const COLD_MARK: &str = "LYCORIS_COLD.txt";

/// One removable drive the user could pick as the cold wallet.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbDrive {
    /// Mount point / root path, e.g. `E:\` on Windows or `/media/usb` elsewhere.
    pub mount: String,
    /// Best-effort human readable name: drive label, or the mount path.
    pub label: String,
    /// True when the drive already carries a Lycoris cold-wallet marker or
    /// vault file.
    pub has_wallet: bool,
    /// Free space in bytes, when the OS reports it.
    pub free_bytes: Option<u64>,
    /// Total capacity in bytes, when the OS reports it.
    pub total_bytes: Option<u64>,
}

/// Which mount point on this system would the cold vault live at?
pub fn cold_vault_path(mount: &str) -> std::path::PathBuf {
    std::path::Path::new(mount)
        .join("LycorisWallet")
        .join("vault.json")
}

/// Enumerate removable drives. Errors degrade to an empty list: the UI simply
/// shows "no drive found".
pub fn drives() -> Vec<UsbDrive> {
    #[cfg(target_os = "windows")]
    {
        windows_drives()
    }
    #[cfg(not(target_os = "windows"))]
    {
        unix_drives()
    }
}

/* ---------------------------------------------------------------- windows */

#[cfg(target_os = "windows")]
fn windows_drives() -> Vec<UsbDrive> {
    use std::path::Path;

    let mut out = Vec::new();
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
    };
    let letter = |n: u32| char::from_u32(b'A' as u32 + n).unwrap_or('?');

    for n in 0..26u32 {
        let root = format!("{}:\\", letter(n));
        let root_wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let root_pcwstr = windows::core::PCWSTR::from_raw(root_wide.as_ptr());

        // DRIVE_REMOVABLE == 2. Floppy drives also report removable; the free
        // space query failing filters the phantom `A:` entry out in practice.
        unsafe {
            if GetDriveTypeW(root_pcwstr) != 2 {
                continue;
            }
            let mut free: u64 = 0;
            let mut total: u64 = 0;
            let mut _left: u64 = 0;
            if GetDiskFreeSpaceExW(
                root_pcwstr,
                Some(&mut free),
                Some(&mut total),
                Some(&mut _left),
            )
            .is_err()
            {
                continue;
            }

            let mut name = [0u16; 256];
            let mut serial = 0u32;
            let mut max_len = 0u32;
            let mut flags = 0u32;
            let mut fs = [0u16; 64];
            let label = if GetVolumeInformationW(
                root_pcwstr,
                Some(&mut name),
                Some(&mut serial),
                Some(&mut max_len),
                Some(&mut flags),
                Some(&mut fs),
            )
            .is_ok()
            {
                let end = name.iter().position(|c| *c == 0).unwrap_or(0);
                let text = String::from_utf16_lossy(&name[..end]);
                if text.trim().is_empty() {
                    format!("{}: 可移动磁盘", letter(n))
                } else {
                    format!("{}: {}", letter(n), text.trim())
                }
            } else {
                format!("{}: 可移动磁盘", letter(n))
            };

            let mount_path = Path::new(&root);
            let has_wallet = cold_vault_path(&root).exists() || mount_path.join(COLD_MARK).exists();

            out.push(UsbDrive {
                mount: root,
                label,
                has_wallet,
                free_bytes: Some(free),
                total_bytes: Some(total),
            });
        }
    }
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out
}

/* ------------------------------------------------------------------- unix */

#[cfg(not(target_os = "windows"))]
fn unix_drives() -> Vec<UsbDrive> {
    // Portable best-effort: probe well-known mount roots instead of parsing
    // /proc/self/mounts, which needs extra privileges on hardened systems.
    let candidates: Vec<String> = std::fs::read_dir("/media")
        .map(|entries| {
            let mut out = Vec::new();
            for user in entries.flatten() {
                let base = user.path();
                if let Ok(children) = std::fs::read_dir(&base) {
                    for volume in children.flatten() {
                        out.push(volume.path().to_string_lossy().to_string());
                    }
                }
                out.push(base.to_string_lossy().to_string());
            }
            out
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for mount in candidates {
        if !std::path::Path::new(&mount).is_dir() {
            continue;
        }
        let has_wallet = cold_vault_path(&mount).exists()
            || std::path::Path::new(&mount).join(COLD_MARK).exists();
        out.push(UsbDrive {
            mount,
            label: String::new(),
            has_wallet,
            free_bytes: None,
            total_bytes: None,
        });
    }
    out
}
