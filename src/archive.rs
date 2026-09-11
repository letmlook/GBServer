//! 最小 ZIP 归档写入器（stored 方式，零第三方依赖）
//!
//! ## 为什么自己写而不用 `zip` / `flate2`
//!
//! 1. 云录像文件（MP4 / PS 流）本身已是压缩格式，ZIP 的 **stored（不压缩）** 方式
//!    既完全正确、又比 deflate 更快（省掉一次无收益的压缩）。
//! 2. 引入 `zip` crate 会连带 `flate2` 等依赖，在当前构建环境下拉取风险高；
//!    本模块用 200 行覆盖所需的最小实现，无额外依赖。
//!
//! 生成结构遵循 APPNOTE.TXT 4.3：
//! `[local file header + data] * N` → `[central directory entry] * N` → `[EOCD]`
//!
//! 不支持 Zip64：单文件或总体积超过 4 GiB 时**显式报错**，而不是产出损坏的归档。

use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
/// general purpose bit 11：文件名使用 UTF-8 编码
const UTF8_FLAG: u16 = 0x0800;
const METHOD_STORED: u16 = 0;
const VERSION_NEEDED: u16 = 20;

/// 单个条目写入结果
#[derive(Debug, Clone)]
pub struct ZipEntry {
    /// 归档内路径（如 `record/xxx.mp4`）
    pub name: String,
    /// 源文件路径
    pub source: PathBuf,
    /// 原始字节数
    pub size: u64,
    /// 原始文件的 CRC-32
    pub crc32: u32,
}

fn crc_table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    })
}

/// 增量 CRC-32（IEEE 802.3，ZIP 使用）
pub fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    let table = crc_table();
    let mut c = !crc;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

/// 一次性计算 CRC-32
pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(0, data)
}

/// MS-DOS 时间/日期（ZIP 头使用）
fn dos_time_date() -> (u16, u16) {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    let year = now.year().clamp(1980, 2107) as u16;
    let time = ((now.hour() as u16) << 11) | ((now.minute() as u16) << 5) | (now.second() as u16 / 2);
    let date = ((year - 1980) << 9) | ((now.month() as u16) << 5) | (now.day() as u16);
    (time, date)
}

fn write_local_header<W: Write>(
    w: &mut W,
    name: &[u8],
    crc: u32,
    size: u32,
    time: u16,
    date: u16,
) -> io::Result<()> {
    w.write_all(&LOCAL_SIG.to_le_bytes())?;
    w.write_all(&VERSION_NEEDED.to_le_bytes())?;
    w.write_all(&UTF8_FLAG.to_le_bytes())?;
    w.write_all(&METHOD_STORED.to_le_bytes())?;
    w.write_all(&time.to_le_bytes())?;
    w.write_all(&date.to_le_bytes())?;
    w.write_all(&crc.to_le_bytes())?;
    w.write_all(&size.to_le_bytes())?; // compressed size == uncompressed (stored)
    w.write_all(&size.to_le_bytes())?;
    w.write_all(&(name.len() as u16).to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // extra field length
    w.write_all(name)
}

struct CentralRecord {
    name: Vec<u8>,
    crc: u32,
    size: u32,
    offset: u32,
}

fn write_central_entry<W: Write>(
    w: &mut W,
    r: &CentralRecord,
    time: u16,
    date: u16,
) -> io::Result<()> {
    w.write_all(&CENTRAL_SIG.to_le_bytes())?;
    w.write_all(&VERSION_NEEDED.to_le_bytes())?; // version made by
    w.write_all(&VERSION_NEEDED.to_le_bytes())?; // version needed
    w.write_all(&UTF8_FLAG.to_le_bytes())?;
    w.write_all(&METHOD_STORED.to_le_bytes())?;
    w.write_all(&time.to_le_bytes())?;
    w.write_all(&date.to_le_bytes())?;
    w.write_all(&r.crc.to_le_bytes())?;
    w.write_all(&r.size.to_le_bytes())?;
    w.write_all(&r.size.to_le_bytes())?;
    w.write_all(&(r.name.len() as u16).to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // extra len
    w.write_all(&0u16.to_le_bytes())?; // comment len
    w.write_all(&0u16.to_le_bytes())?; // disk number start
    w.write_all(&0u16.to_le_bytes())?; // internal attrs
    w.write_all(&0u32.to_le_bytes())?; // external attrs
    w.write_all(&r.offset.to_le_bytes())?;
    w.write_all(&r.name)
}

/// 将若干本地文件以 stored 方式打包为 ZIP。
///
/// `entries` 为 `(归档内路径, 磁盘路径)`。返回每个成功写入条目的元信息。
///
/// 单条目写入采用「占位头 → 流式写数据并算 CRC → 回填头」的单遍策略，
/// 避免为算 CRC 而把大文件读两遍。
pub fn write_zip_stored(
    entries: &[(String, PathBuf)],
    out_path: &Path,
) -> io::Result<Vec<ZipEntry>> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path)?;
    let mut w = BufWriter::new(file);
    let (time, date) = dos_time_date();
    let mut central: Vec<CentralRecord> = Vec::new();
    let mut results: Vec<ZipEntry> = Vec::new();
    let mut offset: u64 = 0;

    for (name, path) in entries {
        let mut f = File::open(path)?;
        let size = f.metadata()?.len();
        // stored 方式受 32 位长度字段限制，超限直接报错而非产出损坏归档
        if size > u32::MAX as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "文件 {} 超过 4GiB（{} 字节），当前 ZIP 实现不支持 Zip64",
                    path.display(),
                    size
                ),
            ));
        }
        if offset + size + 512 > u32::MAX as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "归档总体积超过 4GiB，当前 ZIP 实现不支持 Zip64",
            ));
        }

        let name_bytes = name.as_bytes().to_vec();
        if name_bytes.len() > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("归档内路径过长: {}", name),
            ));
        }

        let header_pos = offset;
        // 占位头：CRC / size 先写 0，数据写完后回填
        write_local_header(&mut w, &name_bytes, 0, 0, time, date)?;

        let mut crc = 0u32;
        let mut written = 0u64;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            crc = crc32_update(crc, &buf[..n]);
            w.write_all(&buf[..n])?;
            written += n as u64;
        }
        // 理论上读取期间文件可能被截断/追加；以实际写入量为准，保证头与实际一致
        debug_assert_eq!(written, size);

        // 回填 local header 的 CRC 与长度（BufWriter::seek 会先 flush）
        let data_end = w.stream_position()?;
        w.seek(SeekFrom::Start(header_pos + 14))?;
        w.write_all(&crc.to_le_bytes())?;
        w.write_all(&(written as u32).to_le_bytes())?;
        w.write_all(&(written as u32).to_le_bytes())?;
        w.seek(SeekFrom::Start(data_end))?;

        central.push(CentralRecord {
            name: name_bytes,
            crc,
            size: written as u32,
            offset: header_pos as u32,
        });
        results.push(ZipEntry {
            name: name.clone(),
            source: path.clone(),
            size: written,
            crc32: crc,
        });

        offset = data_end;
    }

    // central directory
    let cd_start = offset;
    for r in &central {
        write_central_entry(&mut w, r, time, date)?;
    }
    let cd_end = w.stream_position()?;
    let cd_size = cd_end - cd_start;

    // End of central directory
    w.write_all(&EOCD_SIG.to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // disk number
    w.write_all(&0u16.to_le_bytes())?; // disk with cd
    w.write_all(&(central.len() as u16).to_le_bytes())?;
    w.write_all(&(central.len() as u16).to_le_bytes())?;
    w.write_all(&(cd_size as u32).to_le_bytes())?;
    w.write_all(&(cd_start as u32).to_le_bytes())?;
    w.write_all(&0u16.to_le_bytes())?; // comment length

    w.flush()?;
    Ok(results)
}

/// 读取 ZIP 归档，返回 `(归档内路径, 内容)`，并校验结构自洽。
///
/// 只支持本模块产出的形态（stored、无 Zip64、无加密），因此比通用解压器简单得多。
/// 校验四项一致性：EOCD ↔ central directory ↔ local file header ↔ 数据段 CRC。
///
/// 生产路径不依赖它；它的价值是让「写出的确实是标准 ZIP」成为**可执行的证据**，
/// 而不只是口头声明（另见用 Python `zipfile` / `unzip -t` 做过的外部交叉验证）。
#[cfg(test)]
pub(crate) fn read_zip_stored(path: &Path) -> io::Result<Vec<(String, Vec<u8>)>> {
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
    let raw = std::fs::read(path)?;
    if raw.len() < 22 {
        return Err(bad("文件过短，不是有效 ZIP"));
    }
    let eocd = raw.len() - 22;
    if u32::from_le_bytes(raw[eocd..eocd + 4].try_into().unwrap()) != EOCD_SIG {
        return Err(bad("EOCD 签名不正确"));
    }
    let n = u16::from_le_bytes(raw[eocd + 10..eocd + 12].try_into().unwrap()) as usize;
    let cd_size = u32::from_le_bytes(raw[eocd + 12..eocd + 16].try_into().unwrap()) as usize;
    let cd_off = u32::from_le_bytes(raw[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    if cd_off + cd_size != eocd {
        return Err(bad("central directory 偏移与 EOCD 不自洽"));
    }

    let mut out = Vec::with_capacity(n);
    let mut p = cd_off;
    for _ in 0..n {
        if p + 46 > raw.len()
            || u32::from_le_bytes(raw[p..p + 4].try_into().unwrap()) != CENTRAL_SIG
        {
            return Err(bad("central directory 条目签名不正确"));
        }
        let crc = u32::from_le_bytes(raw[p + 16..p + 20].try_into().unwrap());
        let csize = u32::from_le_bytes(raw[p + 20..p + 24].try_into().unwrap());
        let usize_ = u32::from_le_bytes(raw[p + 24..p + 28].try_into().unwrap());
        let nlen = u16::from_le_bytes(raw[p + 28..p + 30].try_into().unwrap()) as usize;
        let elen = u16::from_le_bytes(raw[p + 30..p + 32].try_into().unwrap()) as usize;
        let clen = u16::from_le_bytes(raw[p + 32..p + 34].try_into().unwrap()) as usize;
        let loff = u32::from_le_bytes(raw[p + 42..p + 46].try_into().unwrap()) as usize;
        if csize != usize_ {
            return Err(bad("stored 方式下压缩/未压缩长度必须相等"));
        }
        let name_end = p + 46 + nlen;
        if name_end > raw.len() {
            return Err(bad("文件名越界"));
        }
        let name = String::from_utf8(raw[p + 46..name_end].to_vec())
            .map_err(|_| bad("文件名不是合法 UTF-8"))?;

        if loff + 30 > raw.len()
            || u32::from_le_bytes(raw[loff..loff + 4].try_into().unwrap()) != LOCAL_SIG
        {
            return Err(bad("local file header 签名不正确"));
        }
        let l_crc = u32::from_le_bytes(raw[loff + 14..loff + 18].try_into().unwrap());
        let l_nlen = u16::from_le_bytes(raw[loff + 26..loff + 28].try_into().unwrap()) as usize;
        let l_elen = u16::from_le_bytes(raw[loff + 28..loff + 30].try_into().unwrap()) as usize;
        if l_crc != crc {
            return Err(bad("local header 与 central directory 的 CRC 不一致"));
        }
        let data_start = loff + 30 + l_nlen + l_elen;
        let data_end = data_start + usize_ as usize;
        if data_end > raw.len() {
            return Err(bad("数据段越界"));
        }
        let data = raw[data_start..data_end].to_vec();
        if crc32(&data) != crc {
            return Err(bad("数据段 CRC 与头部声明不一致"));
        }
        out.push((name, data));
        p += 46 + nlen + elen + clen;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_known_vectors() {
        // 标准测试向量
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn test_crc32_incremental_matches_oneshot() {
        let data = b"hello zip world, this is a chunked crc test";
        let one = crc32(data);
        let mut acc = 0u32;
        for chunk in data.chunks(7) {
            acc = crc32_update(acc, chunk);
        }
        assert_eq!(one, acc, "分块计算的 CRC 必须与一次性计算一致");
    }


    #[test]
    fn test_write_zip_stored_roundtrip() {
        let dir = std::env::temp_dir().join(format!("gbserver-zip-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let a_bytes = b"fake-mp4-payload-AAA";
        let b_bytes = b"another-payload-BBBBBBBB";
        let a = dir.join("a.mp4");
        let b = dir.join("b.mp4");
        std::fs::write(&a, a_bytes).unwrap();
        std::fs::write(&b, b_bytes).unwrap();

        let out = dir.join("out.zip");
        let entries = vec![
            ("record/a.mp4".to_string(), a.clone()),
            ("record/b.mp4".to_string(), b.clone()),
        ];
        let written = write_zip_stored(&entries, &out).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(written[0].size, a_bytes.len() as u64);
        assert_eq!(written[1].size, b_bytes.len() as u64);

        let back = read_zip_stored(&out).expect("产出的 ZIP 应可被解析且结构自洽");
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].0, "record/a.mp4");
        assert_eq!(back[0].1, a_bytes);
        assert_eq!(back[1].0, "record/b.mp4");
        assert_eq!(back[1].1, b_bytes);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_zip_stored_handles_utf8_names_and_empty_file() {
        let dir = std::env::temp_dir().join(format!("gbserver-zip-test-u8-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let empty = dir.join("empty.mp4");
        std::fs::write(&empty, b"").unwrap();
        let out = dir.join("out.zip");
        let entries = vec![("录像/空文件.mp4".to_string(), empty.clone())];
        write_zip_stored(&entries, &out).unwrap();

        let back = read_zip_stored(&out).expect("产出的 ZIP 应可被解析且结构自洽");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, "录像/空文件.mp4");
        assert!(back[0].1.is_empty());
        assert_eq!(crc32(b""), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_zip_stored_missing_file_is_error() {
        let dir = std::env::temp_dir().join(format!("gbserver-zip-test-miss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.zip");
        let entries = vec![("x.mp4".to_string(), dir.join("does-not-exist.mp4"))];
        assert!(write_zip_stored(&entries, &out).is_err(), "缺失源文件必须报错");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
