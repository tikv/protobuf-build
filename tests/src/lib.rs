// Copyright 2019 PingCAP, Inc.

#[cfg(feature = "prost-codec")]
extern crate prost_current as prost;
#[cfg(feature = "grpcio-prost-codec")]
extern crate prost_legacy as prost;

#[allow(dead_code)]
#[allow(unknown_lints)]
#[allow(clippy::all)]
#[allow(renamed_and_removed_lints)]
#[allow(bare_trait_objects)]
#[allow(unused_parens)]
#[allow(mismatched_lifetime_syntaxes)]
mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
}

#[cfg(all(test, any(feature = "prost-codec", feature = "grpcio-prost-codec")))]
mod tests {
    use super::protos::compat::{Record, Status};
    use prost::Message;

    #[test]
    fn enum_accessors_and_message_round_trip() {
        let mut record = Record::default();
        assert_eq!(record.get_status(), Status::Unknown);
        record.set_status(Status::Ready);
        record.set_required_status(Status::Ready);
        record.set_payload(b"payload".to_vec());
        let decoded = Record::decode(record.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, record);
        assert_eq!(decoded.get_status(), Status::Ready);
        assert_eq!(decoded.get_required_status(), Status::Ready);
        assert_eq!(decoded.get_payload(), b"payload");
    }

    #[test]
    #[should_panic(expected = "Unknown enum variant: 42")]
    fn optional_enum_accessor_rejects_unknown_values() {
        let record = Record {
            status: Some(42),
            ..Record::default()
        };
        record.get_status();
    }

    #[test]
    #[should_panic(expected = "Unknown enum variant: 42")]
    fn required_enum_accessor_rejects_unknown_values() {
        let record = Record {
            required_status: 42,
            ..Record::default()
        };
        record.get_required_status();
    }
}
