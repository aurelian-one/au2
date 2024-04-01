use std::collections::HashMap;
use std::fmt::Debug;

use automerge::{AutoCommit, Automerge, ObjType, ScalarValue, Value};
use automerge::ReadDoc;
use automerge::transaction::Transactable;
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime, Time};

use crate::decode::*;
use crate::error::AuError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Item {
    // id is the unique id of the item itself.
    pub id: String,
    // at is the timestamp at which the item was created or last modified. To find a history of updates, walk through the graph of changes to this node.
    pub at: OffsetDateTime,
    // class is the specialisation of the item, generic by default, this may translate to an icon or rendering style.
    pub class: Option<String>,
    // content type is how the content should be treated. this is mandatory. text types may be rendered in the UI, while other types may only be downloaded as attachments.
    pub content_type: String,
    // content is the raw content bytes, depending on the content_type this may be utf-8 encoded text or generic bytes.
    pub content: Vec<u8>,
    // rank is the rank of the item among its siblings (those with the same parent id). higher rank should be displayed with higher priority.
    pub rank: i64,
    // parent is the optional parent id which this item is nested under.
    pub parent: Option<String>,
    // deleted is whether this item has been soft deleted. Because auto merge is purely additive, this is no more expensive than removing the item from the tree and helps
    // to ensure we don't re-use deleted ids, and can still search through soft deleted history. NOTE that this shouldn't be used for marking todos as completed, that should be
    // done using a class specialisation.
    pub deleted: bool,
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

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Project {
    pub children: HashMap<String, Item>,
}

impl Project {

    pub fn with_item(&mut self, item: Item, doc: &mut AutoCommit) -> Result<&mut Project, Box<dyn std::error::Error>> {
        if item.id.is_empty() {
            return Err(Box::new(AuError::InvalidField("id".to_string(), "empty".to_string())))
        } else if self.children.contains_key(item.id.as_str()) {
            return Err(Box::new(AuError::InvalidField("id".to_string(), "duplicate key".to_string())))
        } else if let Some(ref parent) = item.parent {
            if !self.children.contains_key(parent.as_str()) {
                return Err(Box::new(AuError::InvalidField("parent".to_string(), "does not exist".to_string())))
            }
        }
        let items_node = find_items_node(doc.document())?;
        let ex_id = doc.put_object(items_node, item.id.clone(), ObjType::Map)?;
        doc.put(&ex_id, "at", ScalarValue::Timestamp((item.at.unix_timestamp_nanos() / 1_000_000) as i64))?;
        doc.put(&ex_id, "content_type", ScalarValue::Str(item.content_type.clone().into()))?;
        doc.put(&ex_id, "rank", ScalarValue::Int(item.rank))?;


        if item.content_type.starts_with("text/") {
            let text_ex_id = doc.put_object(&ex_id, "content", ObjType::Text)?;
            doc.update_text(&text_ex_id, std::str::from_utf8(item.content.as_slice())?)?
        } else {
            doc.put(&ex_id, "content", ScalarValue::Bytes(item.content.clone()))?
        }

        if let Some(ref p) = item.parent {
            doc.put(&ex_id, "parent", ScalarValue::Str(p.clone().into()))?
        }
        if let Some(ref c) = item.class {
            doc.put(&ex_id, "class", ScalarValue::Str(c.clone().into()))?
        }
        if item.deleted {
            doc.put(&ex_id, "deleted", ScalarValue::Boolean(true))?
        }

        self.children.insert(item.id.clone(), item.clone());
        return Ok(self)
    }

}

fn decode_item_inner(
    source: &Automerge,
    item_node: &automerge::ObjId,
    k: &str,
) -> Result<Option<Item>, Box<dyn std::error::Error>> {
    let mut new_item = Item::default();
    new_item.id = k.to_string();

    // required fields
    new_item.at = decode_timestamp(&source, &item_node, "at")?
        .map_or_else(|| Err(Box::new(AuError::NoSuchKey(String::from("at")))), Ok)?;
    new_item.content = decode_content(&source, &item_node, "content")?
        .map_or_else(|| Err(Box::new(AuError::NoSuchKey(String::from("content")))), Ok)?;
    new_item.content_type = decode_string(&source, &item_node, "content_type")?.map_or_else(
        || Err(Box::new(AuError::NoSuchKey(String::from("content_type")))),
        Ok,
    )?;

    // optional fields
    new_item.parent = decode_string(&source, &item_node, "parent")?;
    new_item.class = decode_string(&source, &item_node, "class")?;
    new_item.deleted = decode_bool(&source, &item_node, "deleted")?.unwrap_or(false);
    new_item.rank = decode_i64(&source, &item_node, "rank")?.unwrap_or(0);

    return Ok(Some(new_item));
}

pub fn decode_item(
    source: &Automerge,
    items_node: &automerge::ObjId,
    k: &str,
) -> Result<Option<Item>, Box<dyn std::error::Error>> {
    let item_node = match source.get(items_node, k) {
        Ok(Some((Value::Object(ObjType::Map), n))) => n,
        Ok(Some(_)) => return Err(Box::new(AuError::IncorrectType(
            String::from(k),
            String::from("map"),
        ))),
        Ok(None) => return Ok(None),
        Err(e) => return Err(Box::new(e)),
    };
    match decode_item_inner(source, &item_node, k) {
        Ok(v) => Ok(v),
        Err(e) => Err(Box::new(AuError::NestedError(k.to_string(), e))),
    }
}

fn find_items_node(doc: &Automerge) -> Result<automerge::ObjId, Box<dyn std::error::Error>> {
    match doc.get(automerge::ROOT, "items") {
        Ok(Some((Value::Object(ObjType::Map), n))) => Ok(n),
        Ok(Some(_)) => return Err(Box::new(AuError::IncorrectType("items".to_string(), "map".to_string()))),
        Ok(None) => return Err(Box::new(AuError::NoSuchKey(String::from("items")))),
        Err(e) => return Err(Box::new(AuError::NestedError("items".to_string(), Box::new(e))))
    }
}

pub fn decode_project(source: &Automerge) -> Result<Project, Box<dyn std::error::Error>> {
    let items_node = find_items_node(source)?;
    let mut out = HashMap::new();
    let keys = source.keys(&items_node);
    for k in keys {
        let new_item = decode_item(source, &items_node, k.as_str())
            .map_err(|e| Box::new(AuError::NestedError("items".to_string(), e)))?;
        if new_item.is_none() {
            return Err(Box::new(AuError::NoSuchKey(k.clone())));
        }
        out.insert(k, new_item.unwrap());
    }

    return Ok(Project { children: out });
}


#[cfg(test)]
mod tests {
    use automerge::{AutoCommit, ObjType, ScalarValue};
    use automerge::transaction::Transactable;

    use crate::item::{decode_item, decode_project};

    #[test]
    fn test_decode_empty() {
        let mut doc = AutoCommit::new();
        doc.put_object(automerge::ROOT, "items", ObjType::Map).unwrap();
        let res = decode_project(&doc.document()).expect("failed to decode");
        assert_eq!(res.children.len(), 0);
    }

    #[test]
    fn test_decode_item_missing() {
        let mut doc = AutoCommit::new();
        let res = decode_item(doc.document(), &automerge::ROOT, "thing");
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_decode_item_bad_type() {
        let mut doc = AutoCommit::new();
        doc.put_object(automerge::ROOT, "some-id", ObjType::List).unwrap();
        let res = decode_item(doc.document(), &automerge::ROOT, "some-id");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'some-id': incorrect type, expected map");
    }

    #[test]
    fn test_decode_item_missing_at() {
        let mut doc = AutoCommit::new();
        doc.put_object(automerge::ROOT, "some-id", ObjType::Map).unwrap();
        let res = decode_item(doc.document(), &automerge::ROOT, "some-id");
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'some-id': 'at': no such key");
    }

    #[test]
    fn test_decode_item_minimal() {
        let mut doc = AutoCommit::new();
        let ex_id = doc.put_object(automerge::ROOT, "some-id", ObjType::Map).unwrap();
        doc.put(&ex_id, "at", ScalarValue::Timestamp(1_711_959_236_000)).unwrap();
        doc.put(&ex_id, "content_type", ScalarValue::Str("text/markdown".into())).unwrap();
        let content_ex_id = doc.put_object(&ex_id, "content", ObjType::Text).unwrap();
        doc.update_text(&content_ex_id, "blah blah").unwrap();

        let res = decode_item(doc.document(), &automerge::ROOT, "some-id");
        assert!(res.is_ok());
        let item = res.unwrap().unwrap();
        assert_eq!(item.id, "some-id");
        assert_eq!(item.at.year(), 2024);
        assert_eq!(item.content_type, "text/markdown");
        assert_eq!(item.content.len(), 9);
        assert_eq!(item.deleted, false);
        assert_eq!(item.rank, 0);
        assert!(item.class.is_none());
        assert!(item.parent.is_none());
    }

    #[test]
    fn test_decode_item_full() {
        let mut doc = AutoCommit::new();
        let ex_id = doc.put_object(automerge::ROOT, "some-id", ObjType::Map).unwrap();
        doc.put(&ex_id, "at", ScalarValue::Timestamp(1_711_959_236_000)).unwrap();
        doc.put(&ex_id, "content_type", ScalarValue::Str("text/markdown".into())).unwrap();
        doc.put(&ex_id, "parent", ScalarValue::Str("other-id".into())).unwrap();
        doc.put(&ex_id, "class", ScalarValue::Str("todo".into())).unwrap();
        doc.put(&ex_id, "rank", ScalarValue::Int(42)).unwrap();
        doc.put(&ex_id, "deleted", ScalarValue::Boolean(true)).unwrap();
        doc.put(&ex_id, "content", ScalarValue::Bytes(vec![])).unwrap();

        let res = decode_item(doc.document(), &automerge::ROOT, "some-id");
        assert!(res.is_ok());
        let item = res.unwrap().unwrap();
        assert_eq!(item.id, "some-id");
        assert_eq!(item.at.year(), 2024);
        assert_eq!(item.content_type, "text/markdown");
        assert_eq!(item.content.len(), 0);
        assert_eq!(item.deleted, true);
        assert_eq!(item.rank, 42);
        assert!(item.class.is_some());
        assert_eq!(item.class.unwrap(), "todo");
        assert_eq!(item.parent.unwrap(), "other-id");
    }

    #[test]
    fn test_decode_project_empty() {
        let mut doc = AutoCommit::new();
        doc.put_object(automerge::ROOT, "items", ObjType::Map).unwrap();
        let project = decode_project(doc.document()).unwrap();
        assert_eq!(project.children.len(), 0);
    }

    #[test]
    fn test_decode_project_missing_items() {
        let mut doc = AutoCommit::new();
        let res = decode_project(doc.document());
        assert!(res.is_err());
        assert_eq!(res.err().unwrap().to_string(), "'items': no such key");
    }

    #[test]
    fn test_decode_project_some() {
        let mut doc = AutoCommit::new();
        let items_ex_id = doc.put_object(automerge::ROOT, "items", ObjType::Map).unwrap();

        let ex_id = doc.put_object(items_ex_id, "some-id", ObjType::Map).unwrap();
        doc.put(&ex_id, "at", ScalarValue::Timestamp(1_711_959_236_000)).unwrap();
        doc.put(&ex_id, "content_type", ScalarValue::Str("text/markdown".into())).unwrap();
        let content_ex_id = doc.put_object(&ex_id, "content", ObjType::Text).unwrap();
        doc.update_text(&content_ex_id, "blah blah").unwrap();

        let project = decode_project(doc.document()).unwrap();
        assert_eq!(project.children.len(), 1);
    }
}