//! 用同一协议演示分开的预览和原件、共用一份字节、只有预览三种平台能力。
use hhh_im_protocol::method::{ResourcesDescribeParams, ResourcesReadParams};
use hhh_im_protocol::types::Message;
use hhh_im_protocol::{ErrorKind, ResourceKind, ResourceVariant, ResourceVariants, RpcError};
use serde_json::{Value, json};

const PREVIEW: &[u8] = include_bytes!("../assets/image-preview.png");
const ORIGINAL: &[u8] = include_bytes!("../assets/image-original.png");

fn missing() -> RpcError {
    RpcError::from_kind(ErrorKind::NotFound, "示例 IM 没有这个图片版本。")
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
