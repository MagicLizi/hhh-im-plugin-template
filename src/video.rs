//! 视频示例：演示视频怎么交给宿主（封面、时长、低清版与原视频、分段读），宿主据此点了就播、边下边播。
//!
//! 五个视频各在一条消息里：
//! - `phone`：手机拍的那种——平台另转了一份能播的低清版（`mp4_low`，480×270），原视频是 1280×720；宿主先播低清版，
//!   “显示原视频”再下原视频。
//! - `big`：30 MB 的大视频，索引（`moov`）在开头：宿主不自动下，点了先读开头就能播；往后拖到还没下到的地方，
//!   能从中间读的平台直接读那一段。
//! - `tail`：同样 30 MB，索引在文件末尾：点了先读开头、再从末尾读索引。
//! - `unknown`：索引在末尾、消息里不给大小：宿主先自动下，读到总大小超过上限就停下。
//! - `small`：没封面、不给时长的小视频：自动下好，宿主取第一帧当封面、按文件读时长。
//!
//! 画面是 ffmpeg 生成的测试图案（`assets/video-*`）。大视频不另带 30 MB 的文件：把 8 秒小视频的媒体数据从中间一块
//! 的边界处拉开、填一段全零（索引里后半的块位置跟着往后挪，播放器不读这段零），读的时候现算，不放进内存。这样画面
//! 后半段真的在文件靠后的位置，往后拖要读远处的字节。每段读都按平台网速稍等一下（约 2 MB/s），好看到进度。
//! 要更大的（量宿主下载几百 MB 时的内存）设环境变量 `DEMO_IM_BIG_MB`（21–2000）。
use std::sync::OnceLock;
use std::time::Duration;

use hhh_im_protocol::method::{ResourcesDescribeParams, ResourcesReadParams};
use hhh_im_protocol::types::{Message, Resource, ResourceCover};
use hhh_im_protocol::{ErrorKind, ResourceKind, RpcError};
use serde_json::{Value, json};

const LOW: &[u8] = include_bytes!("../assets/video-low.mp4");
const ORIGINAL: &[u8] = include_bytes!("../assets/video-original.mp4");
const FASTSTART: &[u8] = include_bytes!("../assets/video-faststart.mp4");
const MOOV_AT_END: &[u8] = include_bytes!("../assets/video-moov-end.mp4");
const COVER: &[u8] = include_bytes!("../assets/video-cover.jpg");

/// 文件按多大一段读（`initialize` 里声明给宿主；宿主据此能从任意位置读、断了接着下）。
pub const SEGMENT_BYTES: u64 = 256 * 1024;
/// 大视频凑成多大：30 MB，超过宿主自动下载的上限（20 MB）。
const BIG_BYTES: u64 = 30 * 1024 * 1024;
/// 封面那一份预览图的版本编号。
const COVER_VARIANT: &str = "cover";
/// 这些消息的编号都以它开头（`main.rs` 按它把读请求分过来）。
pub const PREFIX: &str = "video-";

/// 一个视频示例：资源键、消息编号、文件名、有没有封面、要不要在消息里给大小、时长。
struct Sample {
    key: &'static str,
    message: &'static str,
    name: &'static str,
    cover: bool,
    bytes_in_message: bool,
    duration_ms: Option<u64>,
}

const SAMPLES: [Sample; 5] = [
    Sample {
        key: "phone",
        message: "video-phone",
        name: "手机拍的视频.mp4",
        cover: true,
        bytes_in_message: true,
        duration_ms: Some(8000),
    },
    Sample {
        key: "big",
        message: "video-big",
        name: "大视频（索引在开头）.mp4",
        cover: true,
        bytes_in_message: true,
        duration_ms: Some(8000),
    },
    Sample {
        key: "tail",
        message: "video-tail",
        name: "大视频（索引在末尾）.mp4",
        cover: true,
        bytes_in_message: true,
        duration_ms: Some(8000),
    },
    Sample {
        key: "unknown",
        message: "video-unknown",
        name: "大视频（不知道大小）.mp4",
        cover: true,
        bytes_in_message: false,
        duration_ms: Some(8000),
    },
    Sample {
        key: "small",
        message: "video-small",
        name: "小视频（没封面）.mp4",
        cover: false,
        bytes_in_message: true,
        duration_ms: None,
    },
];

fn sample(message: &str, key: &str) -> Option<&'static Sample> {
    SAMPLES
        .iter()
        .find(|s| s.message == message && s.key == key)
}

fn missing() -> RpcError {
    RpcError::from_kind(ErrorKind::NotFound, "示例 IM 没有这个视频。")
}

/// 一份要读的字节：几段拼起来（真实数据或一段全零），读哪段现算。
enum Piece {
    Bytes(&'static [u8]),
    Zeros(u64),
}

impl Piece {
    fn len(&self) -> u64 {
        match self {
            Piece::Bytes(b) => b.len() as u64,
            Piece::Zeros(n) => *n,
        }
    }
}

fn be32(file: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(file[at..at + 4].try_into().expect("四个字节"))
}

/// MP4 顶层或容器里的盒子：类型、起点、长度（含 8 字节盒头）。
fn children(file: &[u8], from: usize, to: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut at = from;
    while at + 8 <= to {
        let size = be32(file, at) as usize;
        let kind: [u8; 4] = file[at + 4..at + 8].try_into().expect("四个字节");
        if size < 8 || at + size > to {
            break;
        }
        out.push((kind, at, size));
        at += size;
    }
    out
}

/// 各个 `stco`（块在文件里的位置表）的位置：moov → trak → mdia → minf → stbl → stco。
fn chunk_tables(file: &[u8]) -> Vec<usize> {
    fn walk(file: &[u8], from: usize, to: usize, out: &mut Vec<usize>) {
        for (kind, at, size) in children(file, from, to) {
            match &kind {
                b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
                    walk(file, at + 8, at + size, out)
                }
                b"stco" => out.push(at),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(file, 0, file.len(), &mut out);
    out
}

/// 一张块位置表里每一项的位置（表头 8 字节盒头 + 4 字节版本与标志 + 4 字节项数）。
fn entries(file: &[u8], table: usize) -> impl Iterator<Item = usize> {
    let count = be32(file, table + 12) as usize;
    (0..count).map(move |i| table + 16 + i * 4)
}

/// 拉开的大视频：前半（含改过的索引与媒体数据盒头）、中间的零、后半。
struct Stretched {
    head: Vec<u8>,
    padding: u64,
    tail: Vec<u8>,
}

/// 把 8 秒小视频的媒体数据从中间一块的边界处拉开 `padding` 字节：媒体数据盒（`mdat`）的长度加上它，
/// 位置在拉开处之后的块都往后挪它；播放器按索引找块，不读中间的零。
fn stretch(file: &'static [u8], padding: u64) -> Stretched {
    let tops = children(file, 0, file.len());
    let (_, mdat, mdat_size) = *tops
        .iter()
        .find(|(kind, ..)| kind == b"mdat")
        .expect("示例视频有 mdat");
    let tables = chunk_tables(file);
    let middle = (mdat + mdat_size / 2) as u32;
    let split = tables
        .iter()
        .flat_map(|&t| entries(file, t))
        .map(|p| be32(file, p))
        .filter(|&offset| offset >= middle)
        .min()
        .expect("示例视频后半有块") as usize;
    let mut whole = file.to_vec();
    let grown = u32::try_from(mdat_size as u64 + padding).expect("媒体数据盒不超过 4 GB");
    whole[mdat..mdat + 4].copy_from_slice(&grown.to_be_bytes());
    for p in tables.iter().flat_map(|&t| entries(file, t)) {
        let offset = be32(&whole, p);
        if offset as usize >= split {
            let moved = u32::try_from(offset as u64 + padding).expect("位置不超过 4 GB");
            whole[p..p + 4].copy_from_slice(&moved.to_be_bytes());
        }
    }
    let tail = whole.split_off(split);
    Stretched {
        head: whole,
        padding,
        tail,
    }
}

/// 这次用的空白大小：凑成 30 MB；设了 `DEMO_IM_BIG_MB` 时按它凑成那么大。
fn padding_for(file: &[u8]) -> u64 {
    let total = std::env::var("DEMO_IM_BIG_MB")
        .ok()
        .and_then(|mb| mb.trim().parse::<u64>().ok())
        .filter(|mb| (21..=2000).contains(mb))
        .map_or(BIG_BYTES, |mb| mb * 1024 * 1024);
    total - file.len() as u64
}

fn stretched(index_at_end: bool) -> &'static Stretched {
    static FRONT: OnceLock<Stretched> = OnceLock::new();
    static END: OnceLock<Stretched> = OnceLock::new();
    let (cell, file) = if index_at_end {
        (&END, MOOV_AT_END)
    } else {
        (&FRONT, FASTSTART)
    };
    cell.get_or_init(|| stretch(file, padding_for(file)))
}

fn big(index_at_end: bool) -> Vec<Piece> {
    let s = stretched(index_at_end);
    vec![
        Piece::Bytes(&s.head),
        Piece::Zeros(s.padding),
        Piece::Bytes(&s.tail),
    ]
}

/// 这几段拼起来的总长与 `[offset, offset + length)` 的字节。
fn slice(pieces: &[Piece], offset: u64, length: u64) -> (u64, Vec<u8>) {
    let total: u64 = pieces.iter().map(Piece::len).sum();
    let end = offset.saturating_add(length).min(total);
    let mut out = Vec::with_capacity(end.saturating_sub(offset) as usize);
    let mut start = 0u64;
    for piece in pieces {
        let (from, to) = (start, start + piece.len());
        start = to;
        let (a, b) = (offset.max(from), end.min(to));
        if a >= b {
            continue;
        }
        let (a, b) = ((a - from) as usize, (b - from) as usize);
        match piece {
            Piece::Bytes(bytes) => out.extend_from_slice(&bytes[a..b]),
            Piece::Zeros(_) => out.resize(out.len() + (b - a), 0),
        }
    }
    (total, out)
}

/// 资源键 + 版本 → 字节与每段读之前等多久（模拟平台网速：一段 256 KB 约 150ms）。
fn content(key: &str, variant: Option<&str>) -> Result<(Vec<Piece>, Duration), RpcError> {
    let ms = Duration::from_millis;
    Ok(match (key, variant) {
        ("phone", Some("mp4_low")) => (vec![Piece::Bytes(LOW)], ms(500)),
        ("phone", None | Some("original")) => (vec![Piece::Bytes(ORIGINAL)], ms(100)),
        ("big", None | Some("original")) => (big(false), ms(150)),
        ("tail" | "unknown", None | Some("original")) => (big(true), ms(150)),
        ("small", None | Some("original")) => (vec![Piece::Bytes(LOW)], ms(300)),
        (
            "phone-cover" | "big-cover" | "tail-cover" | "unknown-cover",
            None | Some(COVER_VARIANT),
        ) => (vec![Piece::Bytes(COVER)], ms(0)),
        _ => return Err(missing()),
    })
}

fn total_of(key: &str, variant: Option<&str>) -> u64 {
    content(key, variant)
        .map(|(pieces, _)| pieces.iter().map(Piece::len).sum())
        .unwrap_or(0)
}

/// 说明与五条视频消息。
pub fn messages(chat: &str, bot: &str, now: i64) -> Vec<Message> {
    let intro: Message = serde_json::from_value(json!({
        "id":"video-intro", "conversationId":chat, "createdAtMs":now,
        "sender":{"id":bot,"kind":"bot","name":"示例机器人"},
        "content":[{"type":"text","text":"视频示例：点封面直接播。手机拍的有低清版与原视频；三个 30 MB 的大视频不自动下（索引在开头、在末尾、不给大小）；小视频没封面。"}]
    }))
    .expect("视频说明符合协议");
    let mut out = vec![intro];
    for (i, s) in SAMPLES.iter().enumerate() {
        let resource = Resource {
            key: s.key.into(),
            kind: ResourceKind::File,
            mime: Some("video/mp4".into()),
            bytes: s.bytes_in_message.then(|| total_of(s.key, None)),
            duration_ms: s.duration_ms,
            voice: false,
            cover: s.cover.then(|| ResourceCover {
                key: format!("{}-cover", s.key),
                width: Some(640),
                height: Some(360),
            }),
        };
        out.push(
            serde_json::from_value(json!({
                "id":s.message, "conversationId":chat, "createdAtMs":now + 1 + i as i64,
                "sender":{"id":bot,"kind":"bot","name":"示例机器人"},
                "content":[{"type":"resource","name":s.name,"resource":resource}]
            }))
            .expect("视频示例符合协议"),
        );
    }
    out
}

/// 版本说明：手机拍的有能播的低清版与原视频，其余只有一份。
pub fn describe(value: Value) -> Result<Value, RpcError> {
    let params: ResourcesDescribeParams = hhh_im_protocol::runtime::params(value)?;
    if params.conversation_id != super::DEMO_CHAT {
        return Err(missing());
    }
    // 封面只有一份预览图（不是原图）；声明出来，宿主才知道拿它显示。
    if params.kind == ResourceKind::Image {
        let owner = params.key.strip_suffix("-cover").ok_or_else(missing)?;
        sample(&params.message_id, owner)
            .filter(|s| s.cover)
            .ok_or_else(missing)?;
        return Ok(
            json!({ "variants": [{"id":COVER_VARIANT,"preview":true,"mime":"image/jpeg","bytes":COVER.len(),"width":640,"height":360}] }),
        );
    }
    if params.kind != ResourceKind::File {
        return Err(missing());
    }
    let s = sample(&params.message_id, &params.key).ok_or_else(missing)?;
    Ok(if s.key == "phone" {
        json!({"variants":[
            {"id":"mp4_low","preview":true,"mime":"video/mp4","bytes":LOW.len(),"width":480,"height":270},
            {"id":"original","original":true,"mime":"video/mp4","bytes":ORIGINAL.len(),"width":1280,"height":720,"name":"手机拍的视频（原视频）.mp4"}
        ]})
    } else {
        let mut original =
            json!({"id":"original","original":true,"mime":"video/mp4","name":s.name});
        if s.bytes_in_message {
            original["bytes"] = total_of(s.key, None).into();
        }
        json!({ "variants": [original] })
    })
}

/// 读一段：按请求的位置现算，读之前按示例网速等一下。
pub async fn read(value: Value) -> Result<Value, RpcError> {
    let params: ResourcesReadParams = hhh_im_protocol::runtime::params(value)?;
    if params.conversation_id != super::DEMO_CHAT {
        return Err(missing());
    }
    let cover = params.kind == ResourceKind::Image;
    let owner = if cover {
        params.key.strip_suffix("-cover").unwrap_or("")
    } else {
        params.key.as_str()
    };
    let s = sample(&params.message_id, owner).ok_or_else(missing)?;
    if cover && !s.cover {
        return Err(missing());
    }
    let (pieces, wait) = content(&params.key, params.variant.as_deref())?;
    let length = params.length.min(SEGMENT_BYTES);
    let (total, bytes) = slice(&pieces, params.offset, length);
    tokio::time::sleep(wait).await;
    let mut chunk = hhh_im_protocol::runtime::chunk_of_bytes(&bytes, 0, length);
    chunk.total = Some(total);
    chunk.eof = params.offset.saturating_add(bytes.len() as u64) >= total;
    serde_json::to_value(chunk)
        .map_err(|_| RpcError::from_kind(ErrorKind::ProtocolError, "视频分段写不出"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whole(pieces: &[Piece]) -> Vec<u8> {
        let (total, bytes) = slice(pieces, 0, u64::MAX);
        assert_eq!(bytes.len() as u64, total);
        bytes
    }

    /// 拉开后的文件：每个块（按索引找）开头的字节与原文件那个块一样；返回在 20 MB 之后的块数与总块数。
    fn same_chunks(original: &[u8], stretched: &[u8]) -> (usize, usize) {
        let (a, b) = (chunk_tables(original), chunk_tables(stretched));
        assert_eq!(a.len(), 2, "画面与声音各一张块位置表");
        let (mut far, mut total) = (0, 0);
        for (&ta, &tb) in a.iter().zip(&b) {
            for (pa, pb) in entries(original, ta).zip(entries(stretched, tb)) {
                let (pa, pb) = (be32(original, pa) as usize, be32(stretched, pb) as usize);
                let n = 64.min(original.len() - pa);
                assert_eq!(&original[pa..pa + n], &stretched[pb..pb + n]);
                total += 1;
                if pb > 20 * 1024 * 1024 {
                    far += 1;
                }
            }
        }
        (far, total)
    }

    #[test]
    fn the_big_videos_spread_their_media_over_thirty_megabytes() {
        for (index_at_end, original) in [(false, FASTSTART), (true, MOOV_AT_END)] {
            let bytes = whole(&big(index_at_end));
            assert_eq!(bytes.len() as u64, BIG_BYTES, "凑成 30 MB");
            let tops = children(&bytes, 0, bytes.len());
            let last = tops.last().map(|(k, ..)| *k);
            assert_eq!(last, Some(if index_at_end { *b"moov" } else { *b"mdat" }));
            let (far, total) = same_chunks(original, &bytes);
            assert!(
                far > 0 && far < total,
                "后半的块在 20 MB 之后：{far}/{total}"
            );
        }
    }

    #[test]
    fn the_phone_video_offers_a_playable_low_res_copy_and_a_separate_original() {
        let request = |key: &str, message: &str| json!({"conversationId":super::super::DEMO_CHAT,"messageId":message,"key":key,"kind":"file"});
        let phone = describe(request("phone", "video-phone")).unwrap();
        assert_eq!(phone["variants"][0]["id"], "mp4_low");
        assert_eq!(phone["variants"][0]["preview"], true);
        assert_eq!(phone["variants"][0]["mime"], "video/mp4");
        assert_eq!(phone["variants"][1]["original"], true);
        assert_eq!(phone["variants"][1]["bytes"], ORIGINAL.len());
        let unknown = describe(request("unknown", "video-unknown")).unwrap();
        assert!(
            unknown["variants"][0].get("bytes").is_none(),
            "不知道大小的不给"
        );
        assert!(describe(request("phone", "video-big")).is_err());
        let mut cover = request("phone-cover", "video-phone");
        cover["kind"] = "image".into();
        let described = describe(cover.clone()).unwrap();
        assert_eq!(described["variants"][0]["id"], COVER_VARIANT);
        assert_eq!(described["variants"][0]["preview"], true);
        assert!(
            described["variants"][0].get("original").is_none(),
            "封面不是原图"
        );
        cover["messageId"] = "video-small".into();
        cover["key"] = "small-cover".into();
        assert!(describe(cover).is_err(), "没封面的视频没有封面资源");
        let messages = messages(super::super::DEMO_CHAT, "bot", 1);
        let resource = |i: usize| {
            serde_json::to_value(&messages[i]).unwrap()["content"][0]["resource"].clone()
        };
        assert_eq!(resource(1)["cover"]["key"], "phone-cover");
        assert_eq!(resource(2)["bytes"], BIG_BYTES);
        assert!(resource(4).get("bytes").is_none());
        assert!(resource(5).get("cover").is_none());
        assert_ne!(LOW, ORIGINAL);
    }

    #[tokio::test]
    async fn reads_return_the_requested_version_and_range() {
        let request = |key: &str, message: &str, variant: Option<&str>, offset: u64| {
            let mut v = json!({"conversationId":super::super::DEMO_CHAT,"messageId":message,"key":key,"kind":"file","offset":offset,"length":1_000_000});
            if let Some(variant) = variant {
                v["variant"] = variant.into();
            }
            v
        };
        let low = read(request("phone", "video-phone", Some("mp4_low"), 0))
            .await
            .unwrap();
        assert_eq!(low["total"], LOW.len());
        assert_eq!(low["eof"], true);
        let original = read(request(
            "phone",
            "video-phone",
            Some("original"),
            SEGMENT_BYTES,
        ))
        .await
        .unwrap();
        assert_eq!(original["total"], ORIGINAL.len());
        assert_eq!(original["eof"], false, "一段最多 256 KB");
        let tail = read(request("tail", "video-tail", None, BIG_BYTES - 1000))
            .await
            .unwrap();
        assert_eq!(tail["total"], BIG_BYTES);
        assert_eq!(tail["eof"], true);
        assert!(
            read(request("phone", "video-phone", Some("hls"), 0))
                .await
                .is_err()
        );
        let mut cover = request("phone-cover", "video-phone", None, 0);
        cover["kind"] = "image".into();
        assert_eq!(read(cover).await.unwrap()["total"], COVER.len());
    }
}
