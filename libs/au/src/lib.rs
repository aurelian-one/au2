use std::io::Error;
use std::io::ErrorKind;

use automerge::{Automerge, ObjType, ReadDoc, Value};
use serde::{Serialize, Deserialize};
use serde::de::Unexpected::Option;
use time::{Date, OffsetDateTime, Time};

#[derive(Serialize, Deserialize, Debug)]
pub struct Item {
    pub id: String,
    pub at: OffsetDateTime,
    pub class: Option<String>,
    pub content_type: String,
    pub content: Vec<u8>,
    pub rank: i32,
    pub deleted: bool,
    pub parent: Option<String>,
}

impl Default for Item {
    fn default () -> Item {
        return Item{
            id: "".to_string(),
            at: OffsetDateTime::new_utc(Date::MIN, Time::MIDNIGHT),
            class: None,
            content_type: "".to_string(),
            content: vec![],
            rank: 0,
            deleted: false,
            parent: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Project {
    pub children: Vec<Item>,
}

pub fn decode(source: &Automerge) -> Result<Project, Box<dyn std::error::Error>> {
    let items_node = match source.get(automerge::ROOT, "items") {
        Ok(Some((Value::Object(ObjType::Map), node))) => node,
        _ => return Err(Box::new(Error::new(ErrorKind::InvalidData, "missing 'items' or is not a map"))),
    };

    let mut out = Vec::new();

    let keys = source.keys(&items_node);
    for k in keys {
        let item_node = match source.get(&items_node, k.clone()) {
            Ok(Some((Value::Object(ObjType::Map), node))) => node,
            _ => return Err(Box::new(Error::new(ErrorKind::InvalidData,  format!("missing 'items' {} or is not a map", k)))),
        };
        let mut new_item = Item::default();
        new_item.id = String::from(&k);

        match source.get(&item_node, "at") {
            Ok(Some((Value::Scalar(value), _))) => {
                if value.is_timestamp() {
                    match OffsetDateTime::from_unix_timestamp_nanos(value.to_i64().unwrap() as i128 * 1_000_000) {
                        Ok(v) => {
                            new_item.at = v;
                        },
                        _ => return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} at is not a valid timestamp", k)))),
                    }
                }
            }
            _ => return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} is missing 'at'", k))))
        }

        match source.get(&item_node, "content_type") {
            Ok(Some((Value::Scalar(value), _))) => {
                if value.is_str() {
                    new_item.content_type = value.to_string()
                } else {
                    return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} content_type is not a string", k))))
                }
            }
            _ => return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} is missing 'content_type'", k))))
        }

        match source.get(&item_node, "content") {
            Ok(Some((Value::Object(ObjType::Text), node ))) => {
                match source.text(&node) {
                    Ok(v) => {
                        new_item.content = Vec::from(v)
                    },
                    Err(e) => {
                        return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} invalid text content: {}", k, e))))
                    }
                }
            },
            Ok(Some((Value::Scalar(value), _))) => {
                match value.to_bytes() {
                    Some(b) => {
                        new_item.content = Vec::from(b)
                    },
                    _ => {
                        return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} invalid byte content", k))))
                    }
                }
            }
            _ => return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} is missing 'content'", k))))
        }

        match source.get(&item_node, "parent") {
            Ok(Some((Value::Scalar(value), _))) => {
                if value.is_str() {
                    new_item.parent = Option::Some(value.to_string())
                } else {
                    return Err(Box::new(Error::new(ErrorKind::InvalidData, format!("item {} parent is not a string", k))))
                }
            },
            _ => {}
        }

        out.push(new_item)
    }

    return Ok(Project{children: out})
}