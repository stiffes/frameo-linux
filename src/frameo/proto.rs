// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal native Prost declarations from borje/unframeo's frameo.proto.
//! Field numbers and wire types are retained, including sint64 zigzag IDs.

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetInfo {}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ClientInfo {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub user_account_id: String,
    #[prost(sint64, tag = "3")]
    pub profile_picture_id: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FrameInfo {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub placement: String,
    #[prost(int32, tag = "3")]
    pub screen_width: i32,
    #[prost(int32, tag = "4")]
    pub screen_height: i32,
    #[prost(bool, tag = "8")]
    pub can_view_photos: bool,
    #[prost(bool, tag = "9")]
    pub can_manage_photos: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Media {
    #[prost(int32, tag = "1")]
    pub size: i32,
    #[prost(string, tag = "2")]
    pub caption: String,
    #[prost(float, tag = "3")]
    pub center_point_x: f32,
    #[prost(float, tag = "4")]
    pub center_point_y: f32,
    #[prost(string, tag = "5")]
    pub file_extension: String,
    #[prost(sint64, tag = "6")]
    pub id: i64,
    #[prost(int32, tag = "7")]
    pub media_type: i32,
    #[prost(message, repeated, tag = "8")]
    pub extra: Vec<Extra>,
    #[prost(sint64, tag = "9")]
    pub capture_date: i64,
    #[prost(sint64, tag = "10")]
    pub content_id: i64,
    #[prost(message, optional, tag = "11")]
    pub error: Option<FrameError>,
    #[prost(int32, tag = "12")]
    pub scale_type: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct Extra {
    #[prost(int32, tag = "1")]
    pub size: i32,
    #[prost(string, tag = "2")]
    pub file_extension: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MediaDataSegment {
    #[prost(bytes, tag = "1")]
    pub data: Vec<u8>,
    #[prost(int64, tag = "16")]
    pub requires_acknowledge_receipt_id: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AcknowledgeReceipt {
    #[prost(int64, tag = "1")]
    pub acknowledge_id: i64,
    #[prost(message, optional, tag = "2")]
    pub error: Option<FrameError>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct FrameError {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(int32, tag = "2")]
    pub additional_int: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct RequestPermission {
    #[prost(int32, tag = "1")]
    pub permission: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct GetMedia {
    #[prost(sint64, tag = "1")]
    pub media_id: i64,
    #[prost(int32, tag = "2")]
    pub width: i32,
    #[prost(int32, tag = "3")]
    pub height: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct MultiPartMessage {
    #[prost(sint64, tag = "1")]
    pub message_id: i64,
    #[prost(int32, tag = "2")]
    pub message_size: i32,
    #[prost(int32, tag = "3")]
    pub data_index: i32,
    #[prost(bytes, tag = "4")]
    pub message_data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn request_messages_preserve_upstream_tags() {
        let info = GetInfo {};
        assert_eq!(info.encode_to_vec(), Vec::<u8>::new());
        let client = ClientInfo {
            name: "Framebridge".into(),
            user_account_id: String::new(),
            profile_picture_id: 0,
        };
        assert_eq!(&client.encode_to_vec()[..2], &[0x0a, 0x0b]);
        let get = GetMedia { media_id: 111, width: 0, height: 0 };
        assert_eq!(get.encode_to_vec(), vec![0x08, 0xde, 0x01]);
    }
}
