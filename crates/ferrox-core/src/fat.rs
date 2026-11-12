//! Minimal zero-heap read-only FAT16/FAT32 filesystem reader for microSD `/SOUNDS/`.
//!
//! Designed for `no_std` embedded targets:
//! - No heap allocation (`[u8; 512]` sector buffers only)
//! - Supports MBR-partitioned cards and superfloppy (VBR at LBA 0)
//! - Supports 8.3 short names and VFAT Long File Names (ASCII subset up to 64 chars)
//! - Streams files one 512-byte sector at a time via [`FileCursor`]

/// Filesystem error.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FatError {
    Io,
    InvalidBootSector,
    UnsupportedFormat,
    NotFound,
    NotADirectory,
    IsADirectory,
    CorruptFat,
}

/// Block device abstraction for 512-byte sector reads.
pub trait BlockDevice {
    #[allow(clippy::result_unit_err)]
    fn read_sector(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), ()>;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FatType {
    Fat16,
    Fat32,
}

/// Directory entry metadata returned by [`FatVolume::find_path`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    pub first_cluster: u32,
    pub size: u32,
    pub is_dir: bool,
}

/// Streaming cursor for reading a file sequentially without allocating.
#[derive(Copy, Clone, Debug)]
pub struct FileCursor {
    pub size: u32,
    pub remaining: u32,
    current_cluster: u32,
    sector_in_cluster: u8,
}

impl FileCursor {
    /// Create a cursor for a regular file [`DirEntry`].
    pub fn new(entry: DirEntry) -> Self {
        Self {
            size: entry.size,
            remaining: entry.size,
            current_cluster: entry.first_cluster,
            sector_in_cluster: 0,
        }
    }

    /// Bytes left to read.
    pub fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Read the next up-to-512 bytes of the file into `out`.
    /// Returns `Ok(0)` at EOF.
    pub fn read_next_sector<D: BlockDevice>(
        &mut self,
        vol: &mut FatVolume,
        dev: &mut D,
        out: &mut [u8; 512],
    ) -> Result<usize, FatError> {
        if self.remaining == 0 {
            return Ok(0);
        }
        if self.current_cluster < 2 {
            return Err(FatError::CorruptFat);
        }
        if self.sector_in_cluster >= vol.sectors_per_cluster {
            match vol.next_cluster(dev, self.current_cluster)? {
                Some(next) => {
                    self.current_cluster = next;
                    self.sector_in_cluster = 0;
                }
                None => {
                    self.remaining = 0;
                    return Ok(0);
                }
            }
        }
        let lba = vol.cluster_to_lba(self.current_cluster) + self.sector_in_cluster as u32;
        dev.read_sector(lba, out).map_err(|()| FatError::Io)?;
        self.sector_in_cluster += 1;

        let valid = (self.remaining as usize).min(512);
        self.remaining -= valid as u32;
        Ok(valid)
    }
}

/// Mounted FAT16/FAT32 volume metadata + 1-sector FAT cache.
pub struct FatVolume {
    pub fat_type: FatType,
    pub sectors_per_cluster: u8,
    fat_start_lba: u32,
    root_dir_lba: u32,
    root_dir_sectors: u32,
    root_cluster: u32,
    data_start_lba: u32,
    total_clusters: u32,
    cached_fat_lba: u32,
    fat_cache: [u8; 512],
}

impl FatVolume {
    /// Mount a FAT16/FAT32 volume from `dev` (checking MBR partition 0..3 or superfloppy LBA 0).
    pub fn mount<D: BlockDevice>(dev: &mut D) -> Result<Self, FatError> {
        let mut sec = [0u8; 512];
        dev.read_sector(0, &mut sec).map_err(|()| FatError::Io)?;
        if u16::from_le_bytes([sec[510], sec[511]]) != 0xAA55 {
            return Err(FatError::InvalidBootSector);
        }

        // Check if LBA 0 is already a valid VBR (superfloppy), otherwise inspect MBR partition table.
        let part_lba = if looks_like_bpb(&sec) {
            0
        } else {
            find_fat_partition_lba(&sec).ok_or(FatError::InvalidBootSector)?
        };

        if part_lba != 0 {
            dev.read_sector(part_lba, &mut sec)
                .map_err(|()| FatError::Io)?;
            if u16::from_le_bytes([sec[510], sec[511]]) != 0xAA55 {
                return Err(FatError::InvalidBootSector);
            }
        }

        let bytes_per_sec = u16::from_le_bytes([sec[11], sec[12]]);
        if bytes_per_sec != 512 {
            return Err(FatError::UnsupportedFormat);
        }
        let sectors_per_cluster = sec[13];
        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err(FatError::UnsupportedFormat);
        }
        let reserved_sectors = u16::from_le_bytes([sec[14], sec[15]]) as u32;
        let num_fats = sec[16] as u32;
        if num_fats == 0 {
            return Err(FatError::InvalidBootSector);
        }
        let root_entry_count = u16::from_le_bytes([sec[17], sec[18]]) as u32;
        let tot16 = u16::from_le_bytes([sec[19], sec[20]]) as u32;
        let fat_sz16 = u16::from_le_bytes([sec[22], sec[23]]) as u32;
        let tot32 = u32::from_le_bytes([sec[32], sec[33], sec[34], sec[35]]);
        let fat_sz32 = u32::from_le_bytes([sec[36], sec[37], sec[38], sec[39]]);

        let total_sectors = if tot16 != 0 { tot16 } else { tot32 };
        let fat_size = if fat_sz16 != 0 { fat_sz16 } else { fat_sz32 };
        if total_sectors == 0 || fat_size == 0 {
            return Err(FatError::InvalidBootSector);
        }

        let root_dir_sectors = (root_entry_count * 32).div_ceil(512);
        let fat_start_lba = part_lba + reserved_sectors;
        let root_dir_lba = fat_start_lba + num_fats * fat_size;
        let data_start_lba = root_dir_lba + root_dir_sectors;

        let data_sectors = total_sectors
            .checked_sub(reserved_sectors + num_fats * fat_size + root_dir_sectors)
            .ok_or(FatError::InvalidBootSector)?;
        let total_clusters = data_sectors / sectors_per_cluster as u32;

        let fat_type = if total_clusters < 4085 {
            // FAT12 not needed for SD cards
            return Err(FatError::UnsupportedFormat);
        } else if total_clusters < 65525 {
            FatType::Fat16
        } else {
            FatType::Fat32
        };

        let root_cluster = if fat_type == FatType::Fat32 {
            u32::from_le_bytes([sec[44], sec[45], sec[46], sec[47]])
        } else {
            0
        };

        Ok(Self {
            fat_type,
            sectors_per_cluster,
            fat_start_lba,
            root_dir_lba,
            root_dir_sectors,
            root_cluster,
            data_start_lba,
            total_clusters,
            cached_fat_lba: u32::MAX,
            fat_cache: [0u8; 512],
        })
    }

    #[inline]
    fn cluster_to_lba(&self, cluster: u32) -> u32 {
        self.data_start_lba + (cluster - 2) * self.sectors_per_cluster as u32
    }

    fn read_fat_sector<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        fat_lba: u32,
    ) -> Result<&[u8; 512], FatError> {
        if self.cached_fat_lba != fat_lba {
            dev.read_sector(fat_lba, &mut self.fat_cache)
                .map_err(|()| FatError::Io)?;
            self.cached_fat_lba = fat_lba;
        }
        Ok(&self.fat_cache)
    }

    /// Look up the next cluster in the FAT chain, returning `Ok(None)` on EOC.
    pub fn next_cluster<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        cluster: u32,
    ) -> Result<Option<u32>, FatError> {
        if cluster < 2 || cluster >= self.total_clusters + 2 {
            return Err(FatError::CorruptFat);
        }
        match self.fat_type {
            FatType::Fat16 => {
                let offset = cluster * 2;
                let lba = self.fat_start_lba + offset / 512;
                let idx = (offset % 512) as usize;
                let sec = self.read_fat_sector(dev, lba)?;
                let val = u16::from_le_bytes([sec[idx], sec[idx + 1]]) as u32;
                if val >= 0xFFF8 {
                    Ok(None)
                } else if val < 2 || val == 0xFFF7 {
                    Err(FatError::CorruptFat)
                } else {
                    Ok(Some(val))
                }
            }
            FatType::Fat32 => {
                let offset = cluster * 4;
                let lba = self.fat_start_lba + offset / 512;
                let idx = (offset % 512) as usize;
                let sec = self.read_fat_sector(dev, lba)?;
                let val = u32::from_le_bytes([sec[idx], sec[idx + 1], sec[idx + 2], sec[idx + 3]])
                    & 0x0FFF_FFFF;
                if val >= 0x0FFF_FFF8 {
                    Ok(None)
                } else if val < 2 || val == 0x0FFF_FFF7 {
                    Err(FatError::CorruptFat)
                } else {
                    Ok(Some(val))
                }
            }
        }
    }

    /// Traverse a `/`-delimited path (e.g. `"/SOUNDS/en/0001.wav"` or `"SOUNDS/en/alt.wav"`).
    pub fn find_path<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        path: &str,
    ) -> Result<DirEntry, FatError> {
        let mut current = DirEntry {
            first_cluster: self.root_cluster,
            size: 0,
            is_dir: true,
        };
        let mut is_fat16_root = self.fat_type == FatType::Fat16;

        for component in path.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if !current.is_dir {
                return Err(FatError::NotADirectory);
            }
            current = self.find_in_dir(dev, current.first_cluster, is_fat16_root, component)?;
            is_fat16_root = false;
        }
        Ok(current)
    }

    /// Open a regular file by path and return a [`FileCursor`].
    pub fn open_file<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        path: &str,
    ) -> Result<FileCursor, FatError> {
        let entry = self.find_path(dev, path)?;
        if entry.is_dir {
            return Err(FatError::IsADirectory);
        }
        Ok(FileCursor::new(entry))
    }

    fn find_in_dir<D: BlockDevice>(
        &mut self,
        dev: &mut D,
        dir_cluster: u32,
        is_fat16_root: bool,
        target: &str,
    ) -> Result<DirEntry, FatError> {
        let mut lfn_buf = [0u8; 64];
        let mut lfn_len = 0usize;
        let mut lfn_expected_order = 0u8;
        let mut lfn_checksum = 0u8;
        let mut sec = [0u8; 512];

        if is_fat16_root {
            for s in 0..self.root_dir_sectors {
                dev.read_sector(self.root_dir_lba + s, &mut sec)
                    .map_err(|()| FatError::Io)?;
                if let Some(found) = scan_dir_sector(
                    &sec,
                    target,
                    &mut lfn_buf,
                    &mut lfn_len,
                    &mut lfn_expected_order,
                    &mut lfn_checksum,
                )? {
                    return Ok(found);
                }
            }
            return Err(FatError::NotFound);
        }

        let mut cluster = dir_cluster;
        let mut guard = 0u32;
        while cluster >= 2 && guard < 4096 {
            guard += 1;
            let base_lba = self.cluster_to_lba(cluster);
            for s in 0..self.sectors_per_cluster as u32 {
                dev.read_sector(base_lba + s, &mut sec)
                    .map_err(|()| FatError::Io)?;
                match scan_dir_sector(
                    &sec,
                    target,
                    &mut lfn_buf,
                    &mut lfn_len,
                    &mut lfn_expected_order,
                    &mut lfn_checksum,
                ) {
                    Ok(Some(found)) => return Ok(found),
                    Ok(None) => {}
                    Err(FatError::NotFound) => return Err(FatError::NotFound),
                    Err(e) => return Err(e),
                }
            }
            match self.next_cluster(dev, cluster)? {
                Some(next) => cluster = next,
                None => break,
            }
        }
        Err(FatError::NotFound)
    }
}

/// Scan one 512-byte directory sector (16 entries). Returns `Err(NotFound)` early if 0x00 end marker hit.
fn scan_dir_sector(
    sec: &[u8; 512],
    target: &str,
    lfn_buf: &mut [u8; 64],
    lfn_len: &mut usize,
    lfn_expected_order: &mut u8,
    lfn_checksum: &mut u8,
) -> Result<Option<DirEntry>, FatError> {
    for i in 0..16 {
        let ent = &sec[i * 32..(i + 1) * 32];
        let first = ent[0];
        if first == 0x00 {
            return Err(FatError::NotFound);
        }
        if first == 0xE5 {
            *lfn_len = 0;
            *lfn_expected_order = 0;
            continue;
        }
        let attr = ent[11];
        if attr == 0x0F {
            // VFAT Long File Name entry
            let seq = first & 0x1F;
            let is_last = (first & 0x40) != 0;
            let csum = ent[13];
            if is_last {
                lfn_buf.fill(0);
                *lfn_len = 0;
                *lfn_expected_order = seq;
                *lfn_checksum = csum;
            } else if *lfn_expected_order == 0
                || seq != *lfn_expected_order - 1
                || csum != *lfn_checksum
            {
                *lfn_len = 0;
                *lfn_expected_order = 0;
                continue;
            } else {
                *lfn_expected_order = seq;
            }
            let base = (seq.saturating_sub(1) as usize) * 13;
            extract_lfn_chars(ent, base, lfn_buf, lfn_len);
            continue;
        }

        // Skip volume ID entries
        if (attr & 0x08) != 0 {
            *lfn_len = 0;
            *lfn_expected_order = 0;
            continue;
        }

        let mut short_buf = [0u8; 12];
        let short_len = format_short_name(&ent[0..11], &mut short_buf);
        let short_sum = sfn_checksum(&ent[0..11]);

        let lfn_matches = *lfn_expected_order == 1
            && *lfn_checksum == short_sum
            && *lfn_len > 0
            && eq_ignore_ascii_case(&lfn_buf[..*lfn_len], target.as_bytes());
        let sfn_matches = eq_ignore_ascii_case(&short_buf[..short_len], target.as_bytes());

        *lfn_len = 0;
        *lfn_expected_order = 0;

        if lfn_matches || sfn_matches {
            let hi = u16::from_le_bytes([ent[20], ent[21]]) as u32;
            let lo = u16::from_le_bytes([ent[26], ent[27]]) as u32;
            let first_cluster = (hi << 16) | lo;
            let size = u32::from_le_bytes([ent[28], ent[29], ent[30], ent[31]]);
            return Ok(Some(DirEntry {
                first_cluster,
                size,
                is_dir: (attr & 0x10) != 0,
            }));
        }
    }
    Ok(None)
}

fn extract_lfn_chars(ent: &[u8], base: usize, lfn_buf: &mut [u8; 64], lfn_len: &mut usize) {
    // Offsets of the 13 UCS-2 code units inside a 32-byte LFN entry
    const OFFSETS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
    for (k, &off) in OFFSETS.iter().enumerate() {
        let code = u16::from_le_bytes([ent[off], ent[off + 1]]);
        if code == 0x0000 || code == 0xFFFF {
            break;
        }
        let pos = base + k;
        if pos < lfn_buf.len() {
            lfn_buf[pos] = if code < 0x80 { code as u8 } else { b'?' };
            if pos + 1 > *lfn_len {
                *lfn_len = pos + 1;
            }
        }
    }
}

fn format_short_name(raw: &[u8], out: &mut [u8; 12]) -> usize {
    let mut name_end = 8usize;
    while name_end > 0 && raw[name_end - 1] == b' ' {
        name_end -= 1;
    }
    let mut ext_end = 3usize;
    while ext_end > 0 && raw[8 + ext_end - 1] == b' ' {
        ext_end -= 1;
    }
    let mut n = 0usize;
    for &b in &raw[..name_end] {
        out[n] = b;
        n += 1;
    }
    if ext_end > 0 {
        out[n] = b'.';
        n += 1;
        for &b in &raw[8..8 + ext_end] {
            out[n] = b;
            n += 1;
        }
    }
    n
}

fn sfn_checksum(raw: &[u8]) -> u8 {
    let mut sum = 0u8;
    for &b in &raw[..11] {
        sum = sum.rotate_right(1).wrapping_add(b);
    }
    sum
}

fn eq_ignore_ascii_case(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (&x, &y) in a.iter().zip(b.iter()) {
        if x.to_ascii_lowercase() != y.to_ascii_lowercase() {
            return false;
        }
    }
    true
}

fn looks_like_bpb(sec: &[u8; 512]) -> bool {
    let jmp = sec[0];
    let valid_jmp = jmp == 0xEB || jmp == 0xE9;
    let bps = u16::from_le_bytes([sec[11], sec[12]]);
    let spc = sec[13];
    valid_jmp && bps == 512 && spc != 0 && spc.is_power_of_two() && sec[16] >= 1
}

fn find_fat_partition_lba(mbr: &[u8; 512]) -> Option<u32> {
    for i in 0..4 {
        let off = 446 + i * 16;
        let ptype = mbr[off + 4];
        if matches!(ptype, 0x04 | 0x06 | 0x0B | 0x0C | 0x0E) {
            let lba = u32::from_le_bytes([mbr[off + 8], mbr[off + 9], mbr[off + 10], mbr[off + 11]]);
            if lba > 0 {
                return Some(lba);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemDisk {
        sectors: [[u8; 512]; 128],
    }

    impl MemDisk {
        fn new() -> Self {
            Self {
                sectors: [[0u8; 512]; 128],
            }
        }
    }

    impl BlockDevice for MemDisk {
        fn read_sector(&mut self, lba: u32, buf: &mut [u8; 512]) -> Result<(), ()> {
            let idx = lba as usize;
            if idx >= self.sectors.len() {
                return Err(());
            }
            buf.copy_from_slice(&self.sectors[idx]);
            Ok(())
        }
    }

    fn build_test_fat32_disk() -> MemDisk {
        let mut disk = MemDisk::new();
        // MBR at LBA 0 pointing partition 0 to LBA 2
        disk.sectors[0][446 + 4] = 0x0C;
        disk.sectors[0][446 + 8..446 + 12].copy_from_slice(&2u32.to_le_bytes());
        disk.sectors[0][510] = 0x55;
        disk.sectors[0][511] = 0xAA;

        // VBR at LBA 2: FAT32 (total_clusters >= 65525 in BPB metadata)
        let vbr = &mut disk.sectors[2];
        vbr[0] = 0xEB;
        vbr[11..13].copy_from_slice(&512u16.to_le_bytes());
        vbr[13] = 1; // 1 sector per cluster
        vbr[14..16].copy_from_slice(&2u16.to_le_bytes()); // reserved = 2 (FAT at LBA 4)
        vbr[16] = 1; // 1 FAT
        vbr[32..36].copy_from_slice(&70000u32.to_le_bytes()); // total sectors
        vbr[36..40].copy_from_slice(&600u32.to_le_bytes()); // fat_sz32 (data starts at LBA 2+2+600 = 604 -> wait, keep in 128 sectors!)
        // Adjust so data fits in 128 sectors while total_clusters >= 65525? No, test checks total_clusters = (70000 - 4) / 1 >= 65525!
        // With reserved=2, fat_sz32=2, total_sectors=70000: data_start_lba = 2 + 2 + 2 = 6!
        vbr[36..40].copy_from_slice(&2u32.to_le_bytes());
        vbr[44..48].copy_from_slice(&2u32.to_le_bytes()); // root_cluster = 2 (LBA 6)
        vbr[510] = 0x55;
        vbr[511] = 0xAA;

        // FAT at LBA 4: cluster 2 (root)=EOC, cluster 3 (SOUNDS)=EOC, cluster 4 (en)=EOC, cluster 5->6->EOC (file)
        let fat = &mut disk.sectors[4];
        fat[8..12].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // cl 2
        fat[12..16].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // cl 3
        fat[16..20].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // cl 4
        fat[20..24].copy_from_slice(&6u32.to_le_bytes()); // cl 5 -> 6
        fat[24..28].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes()); // cl 6 -> EOC

        // Cluster 2 (LBA 6): root dir with "SOUNDS" dir at cluster 3
        let root = &mut disk.sectors[6];
        root[0..11].copy_from_slice(b"SOUNDS     ");
        root[11] = 0x10;
        root[26..28].copy_from_slice(&3u16.to_le_bytes());

        // Cluster 3 (LBA 7): SOUNDS dir with "EN" dir at cluster 4
        let sounds = &mut disk.sectors[7];
        sounds[0..11].copy_from_slice(b"EN         ");
        sounds[11] = 0x10;
        sounds[26..28].copy_from_slice(&4u16.to_le_bytes());

        // Cluster 4 (LBA 8): EN dir with LFN "altitude_m.wav" + 8.3 "ALTITU~1.WAV" at cluster 5, size 600
        let en = &mut disk.sectors[8];
        let sfn = *b"ALTITU~1WAV";
        let csum = sfn_checksum(&sfn);
        // LFN entry 2 (last: 0x42) for chars 13..14 ("v")
        en[0] = 0x42;
        en[11] = 0x0F;
        en[13] = csum;
        en[1..3].copy_from_slice(&(b'v' as u16).to_le_bytes());
        en[3..5].copy_from_slice(&0u16.to_le_bytes());
        // LFN entry 1 (0x01) for chars 0..13 ("altitude_m.wa")
        let lfn1 = &mut en[32..64];
        lfn1[0] = 0x01;
        lfn1[11] = 0x0F;
        lfn1[13] = csum;
        let part1 = b"altitude_m.wa";
        const OFFSETS: [usize; 13] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
        for (k, &ch) in part1.iter().enumerate() {
            lfn1[OFFSETS[k]..OFFSETS[k] + 2].copy_from_slice(&(ch as u16).to_le_bytes());
        }
        // SFN entry at index 2 (offset 64)
        let ent = &mut en[64..96];
        ent[0..11].copy_from_slice(&sfn);
        ent[11] = 0x20;
        ent[26..28].copy_from_slice(&5u16.to_le_bytes());
        ent[28..32].copy_from_slice(&600u32.to_le_bytes());

        // File data in cluster 5 (LBA 9) and cluster 6 (LBA 10)
        disk.sectors[9].fill(0xAA);
        disk.sectors[10][..88].fill(0x55);

        disk
    }

    #[test]
    fn mount_and_read_lfn_multicluster_file() {
        let mut disk = build_test_fat32_disk();
        let mut vol = FatVolume::mount(&mut disk).expect("mount");
        assert_eq!(vol.fat_type, FatType::Fat32);

        let mut cursor = vol
            .open_file(&mut disk, "/SOUNDS/en/altitude_m.wav")
            .expect("open LFN");
        assert_eq!(cursor.remaining(), 600);

        let mut buf = [0u8; 512];
        let n1 = cursor.read_next_sector(&mut vol, &mut disk, &mut buf).unwrap();
        assert_eq!(n1, 512);
        assert!(buf.iter().all(|&b| b == 0xAA));

        let n2 = cursor.read_next_sector(&mut vol, &mut disk, &mut buf).unwrap();
        assert_eq!(n2, 88);
        assert!(buf[..88].iter().all(|&b| b == 0x55));

        let n3 = cursor.read_next_sector(&mut vol, &mut disk, &mut buf).unwrap();
        assert_eq!(n3, 0);
    }

    #[test]
    fn missing_file_returns_not_found() {
        let mut disk = build_test_fat32_disk();
        let mut vol = FatVolume::mount(&mut disk).expect("mount");
        assert_eq!(
            vol.open_file(&mut disk, "/SOUNDS/en/missing.wav").unwrap_err(),
            FatError::NotFound
        );
    }
}
