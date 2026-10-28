//! 用同一协议演示分开的预览和原件、共用一份字节、只有预览三种平台能力。
use hhh_im_protocol::method::{ResourcesDescribeParams, ResourcesReadParams};
use hhh_im_protocol::types::{Message, Resource};
use hhh_im_protocol::{ErrorKind, ResourceKind, ResourceVariant, ResourceVariants, RpcError};
use serde_json::{Value, json};

const PREVIEW: &[u8] = include_bytes!("../assets/image-preview.png");
const ORIGINAL: &[u8] = include_bytes!("../assets/image-original.png");
const VOICE: &[u8] = include_bytes!("../assets/audio-voice.wav");
const AUDIO: &[u8] = include_bytes!("../assets/audio-file.wav");

/// 语音身份是平台给的事实；普通音频不能只因名字里有“语音”就标为 voice。
pub fn audio_message(chat: &str, bot: &str, now: i64) -> Message {
    let voice = Resource {
        key: "voice".into(),
        kind: ResourceKind::File,
        mime: Some("audio/wav".into()),
        bytes: Some(VOICE.len() as u64),
        duration_ms: Some(8000),
        voice: true,
        cover: None,
    };
    let audio = Resource {
        key: "audio".into(),
        bytes: Some(AUDIO.len() as u64),
        duration_ms: None,
        voice: false,
        ..voice.clone()
    };
    serde_json::from_value(json!({
        "id":"audio-demo", "conversationId":chat, "createdAtMs":now,
        "sender":{"id":bot,"kind":"bot","name":"示例机器人"},
        "content":[
            {"type":"text","text":"音频示例：点语音条或音频卡直接播放，再点暂停。"},
            {"type":"resource","name":"语音.wav","resource":voice},
            {"type":"resource","name":"录音文件.wav","resource":audio}
        ]
    }))
    .expect("音频示例符合协议")
}

fn audio_bytes(key: &str) -> Result<&'static [u8], RpcError> {
    match key {
        "voice" => Ok(VOICE),
        "audio" => Ok(AUDIO),
        _ => Err(missing()),
    }
}

fn missing() -> RpcError {
    RpcError::from_kind(ErrorKind::NotFound, "示例 IM 没有这个资源版本。")
}

pub fn message(chat: &str, bot: &str, now: i64) -> Message {
    serde_json::from_value(json!({
        "id":"resource-variants", "conversationId":chat, "createdAtMs":now,
        "sender":{"id":bot,"kind":"bot","name":"示例机器人"},
        "content":[
            {"type":"text","text":"图片示例：预览与原件分开、共用一份图片、仅提供预览。"},
            {"type":"resource","resource":{"key":"separate","kind":"image"},"width":640,"height":480},
            {"type":"resource","resource":{"key":"same","kind":"image"},"width":640,"height":480},
            {"type":"resource","resource":{"key":"preview-only","kind":"image"},"width":128,"height":96}
        ]
    })).expect("示例消息符合协议")
}

fn variants(key: &str) -> Result<ResourceVariants, RpcError> {
    let preview = ResourceVariant {
        id: "preview".into(),
        preview: true,
        mime: Some("image/png".into()),
        width: Some(128),
        height: Some(96),
        bytes: Some(PREVIEW.len() as u64),
        ..Default::default()
    };
    let original = ResourceVariant {
        id: "original".into(),
        original: true,
        mime: Some("image/png".into()),
        name: Some("示例原图.png".into()),
        width: Some(640),
        height: Some(480),
        bytes: Some(ORIGINAL.len() as u64),
        ..Default::default()
    };
    Ok(ResourceVariants {
        variants: match key {
            "separate" => vec![preview, original],
            "same" => vec![ResourceVariant {
                preview: true,
                ..original
            }],
            "preview-only" => vec![preview],
            _ => return Err(missing()),
        },
    })
}

pub fn describe(value: Value) -> Result<Value, RpcError> {
    let params: ResourcesDescribeParams = hhh_im_protocol::runtime::params(value)?;
    if params.conversation_id == super::DEMO_CHAT
        && params.message_id == "audio-demo"
        && params.kind == ResourceKind::File
    {
        let bytes = audio_bytes(&params.key)?;
        return Ok(
            json!({"variants":[{"id":"original","original":true,"mime":"audio/wav","bytes":bytes.len(),"name":if params.key=="voice" {"语音.wav"} else {"录音文件.wav"}}]}),
        );
    }
    if params.conversation_id != super::DEMO_CHAT
        || params.message_id != "resource-variants"
        || params.kind != ResourceKind::Image
    {
        return Err(missing());
    }
    Ok(serde_json::to_value(variants(&params.key)?).expect("资源说明符合协议"))
}

pub fn read(value: Value) -> Result<Value, RpcError> {
    let params: ResourcesReadParams = hhh_im_protocol::runtime::params(value)?;
    if params.conversation_id == super::DEMO_CHAT
        && params.message_id == "audio-demo"
        && params.kind == ResourceKind::File
    {
        if params.variant.as_deref().is_some_and(|v| v != "original") {
            return Err(missing());
        }
        return serde_json::to_value(hhh_im_protocol::runtime::chunk_of_bytes(
            audio_bytes(&params.key)?,
            params.offset,
            params.length,
        ))
        .map_err(|_| RpcError::from_kind(ErrorKind::ProtocolError, "音频分段写不出"));
    }
    if params.conversation_id != super::DEMO_CHAT
        || params.message_id != "resource-variants"
        || params.kind != ResourceKind::Image
    {
        return Err(missing());
    }
    let description = variants(&params.key)?;
    let id = params
        .variant
        .as_deref()
        .unwrap_or(if params.key == "preview-only" {
            "preview"
        } else {
            "original"
        });
    if !description.variants.iter().any(|v| v.id == id) {
        return Err(missing());
    }
    let bytes = if id == "preview" { PREVIEW } else { ORIGINAL };
    serde_json::to_value(hhh_im_protocol::runtime::chunk_of_bytes(
        bytes,
        params.offset,
        params.length,
    ))
    .map_err(|_| RpcError::from_kind(ErrorKind::ProtocolError, "资源分段写不出"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_examples_keep_semantics_and_read_only_the_requested_resource() {
        let message =
            serde_json::to_value(audio_message(super::super::DEMO_CHAT, "bot", 1)).unwrap();
        assert_eq!(message["content"][1]["resource"]["voice"], true);
        assert!(message["content"][2]["resource"].get("voice").is_none());
        let mut request = json!({"conversationId":super::super::DEMO_CHAT,"messageId":"audio-demo","key":"voice","kind":"file","offset":0,"length":12});
        let chunk = read(request.clone()).unwrap();
        assert_eq!(chunk["total"], VOICE.len());
        assert_eq!(chunk["eof"], false);
        assert_eq!(chunk["data"].as_str().unwrap().len(), 16);
        assert!(VOICE.starts_with(b"RIFF") && &VOICE[8..12] == b"WAVE");
        request["messageId"] = "another-message".into();
        assert!(read(request).is_err());
    }
    #[test]
    fn resource_examples_cover_separate_shared_and_missing_originals() {
        let separate = variants("separate").unwrap();
        assert!(separate.validate().is_ok());
        assert_ne!(
            separate.preview(512).unwrap().id,
            separate.original().unwrap().id
        );
        let same = variants("same").unwrap();
        assert_eq!(same.preview(512).unwrap().id, same.original().unwrap().id);
        assert!(variants("preview-only").unwrap().original().is_none());
        assert_ne!(PREVIEW, ORIGINAL);
    }
}
