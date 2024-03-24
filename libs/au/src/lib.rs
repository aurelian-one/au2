use std::fmt::Debug;
use std::io::Error;
use std::io::ErrorKind;

use automerge::{Automerge, ObjType, ReadDoc, Value};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, OffsetDateTime, Time};

use crate::AuError::{IncorrectType, NoSuchKey};

#[derive(Serialize, Deserialize, Debug)]
pub struct Item {
    pub id: String,
    pub at: OffsetDateTime,
    pub class: Option<String>,
    pub content_type: String,
    pub content: Vec<u8>,
    pub rank: i64,
    pub deleted: bool,
    pub parent: Option<String>,
}

impl Default for Item {
    fn default() -> Item {
        return Item {
            id: "".to_string(),
            at: OffsetDateTime::new_utc(Date::MIN, Time::MIDNIGHT),
            class: None,
            content_type: "".to_string(),
            content: vec![],
            rank: 0,
            deleted: false,
            parent: None,
        };
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Project {
    pub children: Vec<Item>,
}

#[derive(Error, Debug)]
pub enum AuError {
    #[error("no such key")]
    NoSuchKey(String),
    #[error("incorrect type - expected {1}")]
    IncorrectType(String, String),
    #[error("'{0}': {1}")]
    NestedError(String, Box<dyn std::error::Error>),
}

fn decode_string(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_str() {
                return Err(Box::new(IncorrectType(
                    String::from(k),
                    String::from("string"),
                )));
            }
            return Ok(Some(v.to_string()));
        }
        _ => Ok(None),
    }
}

fn decode_bool(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_boolean() {
                return Err(Box::new(IncorrectType(
                    String::from(k),
                    String::from("bool"),
                )));
            }
            return Ok(v.to_bool());
        }
        _ => Ok(None),
    }
}

fn decode_i64(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<i64>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_int() {
                return Err(Box::new(IncorrectType(
                    String::from(k),
                    String::from("i64"),
                )));
            }
            return Ok(v.to_i64());
        }
        _ => Ok(None),
    }
}

fn decode_timestamp(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<OffsetDateTime>, Box<dyn std::error::Error>> {
    match source.get(node, k) {
        Err(e) => Err(Box::new(e)),
        Ok(Some((Value::Scalar(v), _))) => {
            if !v.is_timestamp() {
                return Err(Box::new(IncorrectType(
                    String::from(k),
                    String::from("timestamp"),
                )));
            }
            return match OffsetDateTime::from_unix_timestamp_nanos(
                v.to_i64().unwrap() as i128 * 1_1000_000,
            ) {
                Ok(t) => Ok(Some(t)),
                Err(_) => Err(Box::new(IncorrectType(
                    String::from(k),
                    String::from("timestamp"),
                ))),
            };
        }
        _ => Ok(None),
    }
}

fn decode_content(
    source: &Automerge,
    node: &automerge::ObjId,
    k: &str,
) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
    return match source.get(node, "content") {
        Ok(Some((Value::Object(ObjType::Text), node))) => match source.text(&node) {
            Ok(v) => Ok(Some(Vec::from(v))),
            Err(_) => Err(Box::new(IncorrectType(
                String::from(k),
                String::from("text"),
            ))),
        },
        Ok(Some((Value::Scalar(value), _))) => match value.to_bytes() {
            Some(b) => Ok(Some(Vec::from(b))),
            None => Err(Box::new(IncorrectType(
                String::from(k),
                String::from("text"),
            ))),
        },
        _ => Ok(None),
    };
}

pub fn decode_item(
    source: &Automerge,
    items_node: &automerge::ObjId,
    k: String,
) -> Result<Option<Item>, Box<dyn std::error::Error>> {
    let mut new_item = Item::default();
    new_item.id = k.clone();

    let item_node = match source.get(items_node, k) {
        Ok(None) => return Ok(None),
        Err(e) => Err(Box::new(e)),
        Ok(Some((_, n))) => Ok(n),
    }?;

    new_item.parent = decode_string(&source, &item_node, "parent")?;
    new_item.class = decode_string(&source, &item_node, "class")?;
    new_item.content_type = match decode_string(&source, &item_node, "content_type")? {
        None => return Err(Box::new(NoSuchKey(String::from("content_type")))),
        Some(v) => v,
    };
    new_item.deleted = decode_bool(&source, &item_node, "deleted")?.unwrap_or(false);
    new_item.rank = decode_i64(&source, &item_node, "rank")?.unwrap_or(0);
    new_item.at = match decode_timestamp(&source, &item_node, "at")? {
        None => return Err(Box::new(NoSuchKey(String::from("at")))),
        Some(v) => v,
    };
    new_item.content = match decode_content(&source, &item_node, "content")? {
        None => return Err(Box::new(NoSuchKey(String::from("content")))),
        Some(v) => v,
    };

    return Ok(Some(new_item));
}

pub fn decode(source: &Automerge) -> Result<Project, Box<dyn std::error::Error>> {
    let items_node = match source.get(automerge::ROOT, "items") {
        Ok(Some((Value::Object(ObjType::Map), node))) => node,
        _ => {
            return Err(Box::new(Error::new(
                ErrorKind::InvalidData,
                "missing 'items' or is not a map",
            )))
        }
    };

    let mut out = Vec::new();

    let keys = source.keys(&items_node);
    for k in keys {
        let new_item = decode_item(source, &items_node, k.clone())
            .map_err(|e| Box::new(AuError::NestedError(k.clone(), e)))?;
        if new_item.is_none() {
            return Err(Box::new(NoSuchKey(k.clone())));
        }
        out.push(new_item.unwrap())
    }

    return Ok(Project { children: out });
}
