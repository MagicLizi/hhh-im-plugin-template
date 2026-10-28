//! 合并转发的完整示例；资源始终由外层消息与本账号的权限读取。
use hhh_im_protocol::method::{ResourcesDescribeParams, ResourcesReadParams};
use hhh_im_protocol::{ErrorKind, ResourceKind, RpcError};
use serde_json::{Value, json};

const IMAGE: &[u8] = include_bytes!("../assets/image-original.png");
const FILE: &[u8] = "这是一份转发记录里的示例文件。\n只读快照不会生成新的会话消息。\n".as_bytes();
// 六十条足够覆盖虚拟列表、内部滚动与嵌套返回。
const DEMO_RECORD_MESSAGES: usize = 60;

pub fn messages(chat: &str, bot: &str, now: i64) -> Vec<Value> {
    let mut snapshots = vec![
        json!({"sender":{"id":"former-a","name":"陈言"},"createdAtMs":now-60000,"content":[{"type":"text","text":"评审材料已经整理好，图片和说明在下面。"}]}),
        json!({"sender":{"id":"former-b","name":"周可"},"createdAtMs":now-55000,"content":[{"type":"resource","resource":{"kind":"image","key":"forward/image"},"width":640,"height":480}]}),
        json!({"sender":{"name":"陈言"},"createdAtMs":now-50000,"content":[{"type":"resource","resource":{"kind":"file","key":"forward/readme"},"name":"记录说明.md"}]}),
        json!({"sender":{"name":"周可"},"createdAtMs":now-45000,"content":[{"type":"forwarded","record":{"title":"更早的讨论","totalCount":1,"messages":[{"sender":{"name":"原讨论的同事"},"content":[{"type":"text","text":"这是保留原发送人和顺序的嵌套记录。"}]}]}}]}),
    ];
    for index in snapshots.len()..DEMO_RECORD_MESSAGES {
        snapshots.push(json!({"sender":{"name":"陈言"},"createdAtMs":now-40000+index as i64,"content":[{"type":"text","text":format!("第 {} 条讨论：向下阅读后进入嵌套记录，返回仍在原处。",index+1)}]}));
    }
    let wrap = |id: &str, record: Value| {
        json!({
            "id":id,"conversationId":chat,"createdAtMs":now,
            "sender":{"id":bot,"kind":"bot","name":"示例机器人"},
            "content":[{"type":"forwarded","record":record}]
        })
    };
    vec![
        wrap(
            "forwarded-complete",
            json!({"title":"产品讨论记录","totalCount":snapshots.len(),"messages":snapshots}),
        ),
        wrap(
            "forwarded-partial",
            json!({"title":"部分内容的记录","totalCount":3,"incomplete":true,"messages":[
                {"sender":{"name":"原发送人"},"incomplete":true,"content":[{"type":"text","text":"这一部分仍可阅读。"},{"type":"unsupported","kind":"permission","label":"原平台未提供的内容"}]},
                {"sender":{},"content":[{"type":"resource","resource":{"kind":"image","key":"forward/missing"}}],"incomplete":true}
            ]}),
        ),
        wrap(
            "forwarded-unavailable",
            json!({"title":"暂时无法读取的记录","totalCount":1,"messages":[],"incomplete":true}),
        ),
    ]
}

fn denied() -> RpcError {
    RpcError::from_kind(
        ErrorKind::NotFound,
        "平台没有提供这部分资源，请在原平台查看。",
    )
}

fn bytes(message: &str, key: &str, kind: ResourceKind) -> Result<&'static [u8], RpcError> {
    match (message, key, kind) {
        ("forwarded-complete", "forward/image", ResourceKind::Image) => Ok(IMAGE),
        ("forwarded-complete", "forward/readme", ResourceKind::File) => Ok(FILE),
        _ => Err(denied()),
    }
}

pub fn describe(value: Value) -> Result<Value, RpcError> {
    let params: ResourcesDescribeParams = hhh_im_protocol::runtime::params(value)?;
    if params.conversation_id != super::DEMO_CHAT {
        return Err(denied());
    }
    let data = bytes(&params.message_id, &params.key, params.kind)?;
    Ok(
        json!({"variants":[{"id":"original","preview":true,"original":true,"bytes":data.len(),
        "mime":if params.kind==ResourceKind::Image {"image/png"} else {"text/plain"} }]}),
    )
}

pub fn read(value: Value) -> Result<Value, RpcError> {
    let params: ResourcesReadParams = hhh_im_protocol::runtime::params(value)?;
    if params.conversation_id != super::DEMO_CHAT
        || params.variant.as_deref().is_some_and(|v| v != "original")
    {
        return Err(denied());
    }
    let data = bytes(&params.message_id, &params.key, params.kind)?;
    serde_json::to_value(hhh_im_protocol::runtime::chunk_of_bytes(
        data,
        params.offset,
        params.length,
    ))
    .map_err(|_| RpcError::from_kind(ErrorKind::ProtocolError, "资源分段写不出"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarded_resources_are_bound_to_the_outer_message_and_kind() {
        assert_eq!(
            bytes("forwarded-complete", "forward/readme", ResourceKind::File).unwrap(),
            FILE
        );
        assert!(bytes("former-a", "forward/readme", ResourceKind::File).is_err());
        assert!(bytes("forwarded-partial", "forward/image", ResourceKind::Image).is_err());
        assert!(bytes("forwarded-complete", "forward/image", ResourceKind::File).is_err());
    }
    #[test]
    fn examples_keep_order_nested_records_and_explicit_missing_content() {
        let messages = messages("c", "b", 42);
        assert_eq!(
            messages[0]["content"][0]["record"]["messages"]
                .as_array()
                .unwrap()
                .len(),
            DEMO_RECORD_MESSAGES
        );
        assert_eq!(
            messages[0]["content"][0]["record"]["messages"][3]["content"][0]["type"],
            "forwarded"
        );
        assert_eq!(messages[1]["content"][0]["record"]["incomplete"], true);
        assert_eq!(messages[2]["content"][0]["record"]["messages"], json!([]));
    }
}
