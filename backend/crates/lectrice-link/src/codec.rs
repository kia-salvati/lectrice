//! Messages ↔ bytes: `postcard`, no trailing bytes, then the bounds check.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{LinkError, LinkResult};
use crate::limits::MAX_MESSAGE_BYTES;
use crate::messages::Validate;

pub(crate) fn encode<T: Serialize>(message: &T) -> LinkResult<Vec<u8>> {
    let bytes = postcard::to_allocvec(message)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(LinkError::LimitExceeded("message size"));
    }
    Ok(bytes)
}

/// Decodes and checks a message from the other side.
pub(crate) fn decode<T: DeserializeOwned + Validate>(bytes: &[u8]) -> LinkResult<T> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(LinkError::LimitExceeded("message size"));
    }
    let (message, rest) = postcard::take_from_bytes::<T>(bytes)?;
    if !rest.is_empty() {
        return Err(LinkError::Protocol("trailing bytes"));
    }
    message.validate()?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{MAX_ITEMS_PER_BATCH, MAX_TEXT_BYTES};
    use crate::messages::*;

    fn item(item_id: u32, text: &str) -> Item {
        Item { item_id, voice_id: "af_heart".into(), text: text.into(), urgent: false }
    }

    fn every_client_message() -> Vec<ClientMessage> {
        vec![
            ClientMessage::Hello(Hello { proto_minor: 0, app_version: "0.4.0".into(), device_name: "Phone".into() }),
            ClientMessage::Open(Open { engine_id: "kokoro-v1.0".into(), revision: "r1".into(), voice_ids: vec!["af_heart".into()] }),
            ClientMessage::Make(Make { batch_id: 7, items: vec![item(1, "A short invented sentence."), Item { urgent: true, ..item(2, "Another one.") }] }),
            ClientMessage::Cancel(CancelScope::Batch(7)),
            ClientMessage::Cancel(CancelScope::All),
            ClientMessage::Ping { t_ms: 12 },
            ClientMessage::Pong { t_ms: 13 },
            ClientMessage::Bye(ByeReason::Idle),
        ]
    }

    fn every_server_message() -> Vec<ServerMessage> {
        let offer = EngineOffer { engine_id: "kokoro-v1.0".into(), revision: "r1".into(), installed: true, sample_rate: 24_000 };
        vec![
            ServerMessage::Welcome(Welcome { proto_minor: 0, app_version: "0.2.0".into(), computer_name: "Laptop".into(), engines: vec![offer] }),
            ServerMessage::Ready(Ready { engine_id: "kokoro-v1.0".into(), revision: "r1".into(), sample_rate: 24_000, voices: vec!["af_heart".into()] }),
            ServerMessage::Refused(Refusal::ModelMissing { bundle_id: "kokoro-en-v1_0-fast".into() }),
            ServerMessage::Refused(Refusal::RevisionMismatch { revision: "r2".into() }),
            ServerMessage::Refused(Refusal::Busy),
            ServerMessage::Clip(ClipChunk { item_id: 1, sample_rate: 24_000, samples: 100, make_ms: 5, seq: 0, last: true, flac: vec![1, 2, 3] }),
            ServerMessage::ItemFailed { item_id: 2, error: ItemError { name: "VoiceUnknown".into(), code: 2001 } },
            ServerMessage::Load(Load { queue_items: 3, own_listener: true }),
            ServerMessage::Ping { t_ms: 1 },
            ServerMessage::Pong { t_ms: 2 },
            ServerMessage::Bye(ByeReason::EngineSwitch),
        ]
    }

    #[test]
    fn every_message_round_trips() {
        for message in every_client_message() {
            assert_eq!(decode::<ClientMessage>(&encode(&message).unwrap()).unwrap(), message);
        }
        for message in every_server_message() {
            assert_eq!(decode::<ServerMessage>(&encode(&message).unwrap()).unwrap(), message);
        }
    }

    #[test]
    fn refuses_trailing_bytes_and_garbage() {
        let mut bytes = encode(&ClientMessage::Ping { t_ms: 1 }).unwrap();
        bytes.push(0);
        assert!(matches!(decode::<ClientMessage>(&bytes), Err(LinkError::Protocol(_))));
        assert!(decode::<ClientMessage>(&[0xff, 0xff, 0xff]).is_err());
        assert!(decode::<ServerMessage>(&[]).is_err());
    }

    #[test]
    fn refuses_too_many_items_and_too_long_text() {
        let many = ClientMessage::Make(Make { batch_id: 1, items: (0..=MAX_ITEMS_PER_BATCH as u32).map(|id| item(id, "Hi.")).collect() });
        assert!(matches!(decode::<ClientMessage>(&encode(&many).unwrap()), Err(LinkError::LimitExceeded(_))));
        let long = ClientMessage::Make(Make { batch_id: 1, items: vec![item(1, &"a".repeat(MAX_TEXT_BYTES + 1))] });
        assert!(matches!(decode::<ClientMessage>(&encode(&long).unwrap()), Err(LinkError::LimitExceeded(_))));
        let empty = ClientMessage::Make(Make { batch_id: 1, items: Vec::new() });
        assert!(decode::<ClientMessage>(&encode(&empty).unwrap()).is_err());
    }

    #[test]
    fn refuses_a_long_device_name_and_a_big_chunk() {
        let hello = ClientMessage::Hello(Hello { proto_minor: 0, app_version: "1".into(), device_name: "x".repeat(200) });
        assert!(decode::<ClientMessage>(&encode(&hello).unwrap()).is_err());
        let chunk = ServerMessage::Clip(ClipChunk { item_id: 1, sample_rate: 24_000, samples: 1, make_ms: 1, seq: 0, last: true, flac: vec![0; 60_001] });
        assert!(decode::<ServerMessage>(&encode(&chunk).unwrap()).is_err());
    }
}
