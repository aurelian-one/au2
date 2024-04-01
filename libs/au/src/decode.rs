use std::borrow::Cow;
use automerge::{Automerge, ObjType, Value};
use time::OffsetDateTime;

use automerge::ReadDoc;
use crate::error::AuError;

pub fn decode_string(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_str() {
                return Err(Box::new(AuError::IncorrectType(
                    String::from(k),
                    String::from("string"),
                )));
            }
            return Ok(v.to_str().map(|sr| sr.to_string()));
        }
        _ => Ok(None),
    }
}

pub fn decode_bool(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) if v.is_boolean() => Ok(v.to_bool()),
        Ok(Some(_)) => Err(Box::new(AuError::IncorrectType(
            String::from(k),
            String::from("bool"),
        ))),
        _ => Ok(None),
    }
}

pub fn decode_i64(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<i64>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_int() {
                return Err(Box::new(AuError::IncorrectType(
                    String::from(k),
                    String::from("i64"),
                )));
            }
            return Ok(v.to_i64());
        }
        _ => Ok(None),
    }
}

pub fn decode_timestamp(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<OffsetDateTime>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_timestamp() {
                return Err(Box::new(AuError::IncorrectType(
                    String::from(k),
                    String::from("timestamp"),
                )));
            }
            return match OffsetDateTime::from_unix_timestamp_nanos(
                v.to_u64().unwrap() as i128 * 1_000_000,
            ) {
                Ok(t) => Ok(Some(t)),
                Err(_) => Err(Box::new(AuError::IncorrectType(
                    String::from(k),
                    String::from("timestamp"),
                ))),
            };
        }
        _ => Ok(None),
    }
}

pub fn decode_content<'a>(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<Cow<'a, [u8]>>, Box<dyn std::error::Error>> {
    return match source.get(node, k) {
        Ok(Some((Value::Object(ObjType::Text), node))) => match source.text(&node) {
            Ok(v) =>  Ok(Some(Cow::from(v.as_bytes().to_vec()))),
            Err(_) => Err(Box::new(AuError::IncorrectType(
                String::from(k),
                String::from("text"),
            ))),
        },
        Ok(Some((Value::Scalar(value), _))) => match value.to_bytes() {
            Some(b) => Ok(Some(Cow::from(b.to_vec()))),
            None => Err(Box::new(AuError::IncorrectType(
                String::from(k),
                String::from("text"),
            ))),
        },
        _ => Ok(None),
    };
}


#[cfg(test)]
mod tests {
    use automerge::{AutoCommit, ObjType, ReadDoc, ScalarValue};
    use automerge::transaction::Transactable;
    use crate::decode::{decode_bool, decode_content, decode_i64, decode_string, decode_timestamp};

    #[test]
    fn test_decode_string_missing() {
        let mut doc = AutoCommit::new();
        let res = decode_string(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_decode_string_bad_type() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", 42).unwrap();
        let res = decode_string(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'thing': incorrect type, expected string");
    }

    #[test]
    fn test_decode_string_some() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", "foo").unwrap();
        let res = decode_string(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert_eq!(res.ok().unwrap().unwrap(), "foo");
    }

    #[test]
    fn test_decode_i64_missing() {
        let mut doc = AutoCommit::new();
        let res = decode_i64(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_decode_i64_bad_type() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", "foo").unwrap();
        let res = decode_i64(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'thing': incorrect type, expected i64");
    }

    #[test]
    fn test_decode_i64_some() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", 42).unwrap();
        let res = decode_i64(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert_eq!(res.ok().unwrap().unwrap(), 42);
    }

    #[test]
    fn test_decode_bool_missing() {
        let mut doc = AutoCommit::new();
        let res = decode_bool(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_decode_bool_bad_type() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", "foo").unwrap();
        let res = decode_bool(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'thing': incorrect type, expected bool");
    }

    #[test]
    fn test_decode_bool_some() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", true).unwrap();
        let res = decode_bool(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert_eq!(res.ok().unwrap().unwrap(), true);
    }

    #[test]
    fn test_decode_timestamp_missing() {
        let mut doc = AutoCommit::new();
        let res = decode_timestamp(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_decode_timestamp_bad_type() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", "foo").unwrap();
        let res = decode_timestamp(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'thing': incorrect type, expected timestamp");
    }

    #[test]
    fn test_decode_timestamp_some() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", ScalarValue::Timestamp(1_711_959_236_000)).unwrap();
        let res = decode_timestamp(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().unwrap().year(), 2024);
    }

    #[test]
    fn test_decode_content_missing() {
        let mut doc = AutoCommit::new();
        let res = decode_content(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_decode_content_bad_type() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", 42).unwrap();
        let res = decode_content(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'thing': incorrect type, expected text");
    }

    #[test]
    fn test_decode_content_some_bytes() {
        let mut doc = AutoCommit::new();
        doc.put(automerge::ROOT, "thing", ScalarValue::Bytes(vec![0, 1, 2])).unwrap();
        let res = decode_content(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert_eq!(res.unwrap().unwrap().len(), 3);
    }

    #[test]
    fn test_decode_content_some_text() {
        let mut doc = AutoCommit::new();
        doc.put_object(automerge::ROOT, "thing", ObjType::Text).unwrap();
        doc.update_text(doc.get(automerge::ROOT, "thing").unwrap().unwrap().1.as_ref(), "hello world").unwrap();
        doc.splice_text(doc.get(automerge::ROOT, "thing").unwrap().unwrap().1.as_ref(), 5, 0,",").unwrap();
        let res = decode_content(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert_eq!(res.unwrap().unwrap().len(), 12);
    }

}