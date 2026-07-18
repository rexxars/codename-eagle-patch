//! Raw CD-DA (Redbook audio) reading on Windows via `DeviceIoControl`.
//!
//! Audio tracks are not files; we read the TOC (`IOCTL_CDROM_READ_TOC`) to find the
//! track layout, then pull raw 2352-byte audio sectors (`IOCTL_CDROM_RAW_READ`,
//! `CDDA` mode). Those bytes are interleaved 16-bit little-endian stereo PCM at
//! 44.1 kHz - exactly what we measure/normalize/encode.

#[cfg(not(windows))]
use std::io;

pub struct Track {
    pub number: u8,
    pub start_lba: u32,
    pub end_lba: u32, // exclusive: start of the next track (or lead-out)
    pub is_audio: bool,
}

#[cfg(windows)]
pub use sys::*;

#[cfg(windows)]
mod sys {
    use super::Track;
    use std::ffi::c_void;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetDriveTypeW, GetLogicalDrives, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const GENERIC_READ: u32 = 0x8000_0000;
    const DRIVE_CDROM: u32 = 5;
    const IOCTL_CDROM_READ_TOC: u32 = 0x0002_4000;
    const IOCTL_CDROM_RAW_READ: u32 = 0x0002_403E;
    const TRACK_MODE_CDDA: u32 = 2;
    const RAW_SECTOR: usize = 2352; // bytes per CD-DA sector
    const SECTORS_PER_READ: u32 = 20; // 20*2352 = 47040 bytes < the ~64 KiB cap

    #[repr(C)]
    struct TrackData {
        reserved: u8,
        control_adr: u8, // low nibble = Control (bit 2 set => data track)
        track_number: u8,
        reserved1: u8,
        address: [u8; 4], // MSF: address[1]=min, [2]=sec, [3]=frame
    }
    #[repr(C)]
    struct CdromToc {
        length: [u8; 2],
        first_track: u8,
        last_track: u8,
        track_data: [TrackData; 100], // tracks then the lead-out entry
    }
    #[repr(C)]
    struct RawReadInfo {
        disk_offset: i64, // byte offset = start_lba * 2048 (logical-sector units)
        sector_count: u32,
        track_mode: u32,
    }

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn msf_to_lba(a: [u8; 4]) -> u32 {
        let (m, s, f) = (a[1] as u32, a[2] as u32, a[3] as u32);
        (m * 60 + s) * 75 + f - 150
    }

    pub struct Cd(HANDLE);
    impl Drop for Cd {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Drive letters whose type is CD-ROM.
    pub fn cdrom_drives() -> Vec<char> {
        let mask = unsafe { GetLogicalDrives() };
        (0..26u32)
            .filter(|i| mask & (1 << i) != 0)
            .filter_map(|i| {
                let letter = (b'A' + i as u8) as char;
                let root = wide(&format!("{letter}:\\"));
                (unsafe { GetDriveTypeW(root.as_ptr()) } == DRIVE_CDROM).then_some(letter)
            })
            .collect()
    }

    pub fn open(letter: char) -> io::Result<Cd> {
        let path = wide(&format!(r"\\.\{letter}:"));
        let h = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Cd(h))
    }

    pub fn read_toc(cd: &Cd) -> io::Result<Vec<Track>> {
        let mut toc: CdromToc = unsafe { std::mem::zeroed() };
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                cd.0,
                IOCTL_CDROM_READ_TOC,
                std::ptr::null(),
                0,
                (&mut toc as *mut CdromToc).cast::<c_void>(),
                std::mem::size_of::<CdromToc>() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let n = (toc.last_track - toc.first_track + 1) as usize;
        let mut tracks = Vec::with_capacity(n);
        for i in 0..n {
            let td = &toc.track_data[i];
            tracks.push(Track {
                number: td.track_number,
                start_lba: msf_to_lba(td.address),
                // the entry after the last track is the lead-out, giving its end
                end_lba: msf_to_lba(toc.track_data[i + 1].address),
                is_audio: td.control_adr & 0x04 == 0,
            });
        }
        Ok(tracks)
    }

    /// Read a track's raw CD-DA as interleaved 16-bit LE stereo PCM.
    pub fn read_audio(cd: &Cd, start_lba: u32, end_lba: u32) -> io::Result<Vec<i16>> {
        let total = end_lba.saturating_sub(start_lba);
        let mut out: Vec<i16> = Vec::with_capacity(total as usize * (RAW_SECTOR / 2));
        let mut buf = vec![0u8; SECTORS_PER_READ as usize * RAW_SECTOR];
        let mut lba = start_lba;
        while lba < end_lba {
            let count = SECTORS_PER_READ.min(end_lba - lba);
            let info = RawReadInfo {
                disk_offset: (lba as i64) * 2048,
                sector_count: count,
                track_mode: TRACK_MODE_CDDA,
            };
            let want = count as usize * RAW_SECTOR;
            let mut returned = 0u32;
            let ok = unsafe {
                DeviceIoControl(
                    cd.0,
                    IOCTL_CDROM_RAW_READ,
                    (&info as *const RawReadInfo).cast::<c_void>(),
                    std::mem::size_of::<RawReadInfo>() as u32,
                    buf.as_mut_ptr().cast::<c_void>(),
                    want as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            for s in buf[..returned as usize].chunks_exact(2) {
                out.push(i16::from_le_bytes([s[0], s[1]]));
            }
            lba += count;
        }
        Ok(out)
    }
}

// Non-Windows stub so the crate type-checks on the host; ripping is Windows-only.
#[cfg(not(windows))]
pub struct Cd;
#[cfg(not(windows))]
pub fn cdrom_drives() -> Vec<char> {
    Vec::new()
}
#[cfg(not(windows))]
pub fn open(_letter: char) -> io::Result<Cd> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "CD ripping is Windows-only"))
}
#[cfg(not(windows))]
pub fn read_toc(_cd: &Cd) -> io::Result<Vec<Track>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "CD ripping is Windows-only"))
}
#[cfg(not(windows))]
pub fn read_audio(_cd: &Cd, _start_lba: u32, _end_lba: u32) -> io::Result<Vec<i16>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "CD ripping is Windows-only"))
}
