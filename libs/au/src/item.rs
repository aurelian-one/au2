use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;

use automerge::transaction::Transactable;
use automerge::ReadDoc;
use automerge::{AutoCommit, Automerge, ObjType, ScalarValue, Value};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use time::OffsetDateTime;

use crate::decode::*;
use crate::error::AuError;

const DOC_ITEMS_NODE: &str = "items";
const DOC_ITEM_ID_NODE: &str = "id";
const DOC_ITEM_PARENT_NODE: &str = "parent";
const DOC_ITEM_AT_NODE: &str = "at";
const DOC_ITEM_CONTENT_NODE: &str = "content";
const DOC_ITEM_CONTENT_TYPE_NODE: &str = "content_type";
const DOC_ITEM_RANK_NODE: &str = "rank";
const DOC_ITEM_CLASS_NODE: &str = "class";
const CONTENT_TYPE_DEFAULT: &str = "text/plain";
const CONTENT_TYPE_DEFAULT_PREFIX: &str = "text/";

// An Item is an item in the hierarchy. We use reference counted strings to avoid specifying lifetimes.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Item {
    // id is the unique id of the item itself.
    pub id: Rc<str>,
    // at is the timestamp at which the item was created or last modified. To find a history of updates, walk through the graph of changes to this node.
    pub at: OffsetDateTime,
    // class is the specialisation of the item, generic by default, this may translate to an icon or rendering style.
    pub class: Option<Rc<str>>,
    // content type is how the content should be treated. this is mandatory. text types may be rendered in the UI, while other types may only be downloaded as attachments.
    pub content_type: Rc<str>,
    // content is the raw content bytes, depending on the content_type this may be utf-8 encoded text or generic bytes.
    pub content: Rc<[u8]>,
    // rank is the rank of the item among its siblings (those with the same parent id). higher rank should be displayed with higher priority.
    pub rank: i64,
    // parent is the optional parent id which this item is nested under.
    pub parent: Option<Rc<str>>,
}

impl Default for Item {
    fn default() -> Item {
        return Item {
            id: Rc::from("default"),
            at: OffsetDateTime::UNIX_EPOCH,
            class: None,
            content_type: Rc::from(CONTENT_TYPE_DEFAULT),
            content: Rc::from(vec![]),
            rank: 0,
            parent: None,
        };
    }
}

pub enum ItemUpdate {
    Parent(Option<Box<str>>),
    Rank(i64),
    Class(Option<Box<str>>),
    Content(Box<str>, Box<[u8]>),
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Project {
    children: HashMap<Box<str>, Rc<Item>>,
}

impl Project {
    pub fn with_item(&mut self, item: &Item, doc: &mut AutoCommit) -> Result<&mut Project, Box<dyn std::error::Error>> {
        if item.id.is_empty() {
            return Err(Box::new(AuError::InvalidField(Box::from(DOC_ITEM_ID_NODE), Box::from("empty"))));
        } else if self.children.contains_key(item.id.as_ref()) {
            return Err(Box::new(AuError::InvalidField(
                Box::from(DOC_ITEM_ID_NODE),
                Box::from("duplicate key"),
            )));
        } else if let Some(ref parent) = item.parent {
            if !self.children.contains_key(parent.as_ref()) {
                return Err(Box::new(AuError::InvalidField(
                    Box::from(DOC_ITEM_PARENT_NODE),
                    Box::from("does not exist"),
                )));
            }
        }
        let items_node = match find_items_node(doc.document()) {
            Ok(n) => n,
            Err(_) => doc.put_object(automerge::ROOT, DOC_ITEMS_NODE, ObjType::Map)?,
        };
        let ex_id = doc.put_object(items_node, item.id.as_ref(), ObjType::Map)?;
        doc.put(
            &ex_id,
            DOC_ITEM_AT_NODE,
            ScalarValue::Timestamp((item.at.unix_timestamp_nanos() / 1_000_000) as i64),
        )?;
        doc.put(
            &ex_id,
            DOC_ITEM_CONTENT_TYPE_NODE,
            ScalarValue::Str(SmolStr::from(item.content_type.as_ref())),
        )?;
        doc.put(&ex_id, DOC_ITEM_RANK_NODE, ScalarValue::Int(item.rank))?;

        if item.content_type.starts_with(CONTENT_TYPE_DEFAULT_PREFIX) {
            let text_ex_id = doc.put_object(&ex_id, DOC_ITEM_CONTENT_NODE, ObjType::Text)?;
            doc.update_text(&text_ex_id, std::str::from_utf8(item.content.as_ref())?)?
        } else {
            doc.put(&ex_id, DOC_ITEM_CONTENT_NODE, ScalarValue::Bytes(item.content.to_vec()))?
        }

        if let Some(ref p) = item.parent {
            doc.put(&ex_id, DOC_ITEM_PARENT_NODE, ScalarValue::Str(SmolStr::from(p.as_ref())))?
        }
        if let Some(ref c) = item.class {
            doc.put(&ex_id, DOC_ITEM_CLASS_NODE, ScalarValue::Str(SmolStr::from(c.as_ref())))?
        }
        self.children.insert(Box::from(item.id.as_ref()), Rc::new(item.clone()));
        Ok(self)
    }

    pub fn without_item(&mut self, id: &str, doc: &mut AutoCommit) -> Result<&mut Project, Box<dyn std::error::Error>> {
        if !self.children.contains_key(id) {
            return Err(Box::new(AuError::NoSuchKey(Box::from(id))));
        } else if self.has_children(Some(id)) {
            return Err(Box::new(AuError::InvalidOperation(Box::from(id), Box::from("has children"))));
        }
        let items_node = match find_items_node(doc.document()) {
            Ok(n) => n,
            Err(_) => doc.put_object(automerge::ROOT, DOC_ITEMS_NODE, ObjType::Map)?,
        };
        if let Err(e) = doc.delete(items_node, id) {
            return Err(Box::new(e));
        }
        self.children.remove(id);
        Ok(self)
    }

    pub fn with_updated_item(
        &mut self,
        id: &str,
        updates: &[ItemUpdate],
        doc: &mut AutoCommit,
    ) -> Result<&mut Project, Box<dyn std::error::Error>> {
        let target_item = match self.children.get(id) {
            Some(p) => p.clone(),
            None => return Err(Box::new(AuError::NoSuchKey(Box::from(id.as_ref())))),
        };

        let items_node = match find_items_node(doc.document()) {
            Ok(n) => n,
            Err(_) => doc.put_object(automerge::ROOT, DOC_ITEMS_NODE, ObjType::Map)?,
        };
        let item_node = match doc.document().get(items_node, id) {
            Ok(Some((Value::Object(ObjType::Map), n))) => n,
            Ok(Some(_)) => return Err(Box::new(AuError::IncorrectType(Box::from(id), Box::from("map")))),
            Ok(None) => return Err(Box::new(AuError::NoSuchKey(Box::from(id)))),
            Err(e) => return Err(Box::new(e)),
        };

        let mut new_item = target_item.as_ref().clone();

        for update in updates {
            match update {
                ItemUpdate::Parent(None) => {
                    if let Some(_) = new_item.parent {
                        new_item.parent = None;
                        doc.delete(&item_node, DOC_ITEM_PARENT_NODE)?;
                    }
                }
                ItemUpdate::Parent(Some(ref new_parent)) => {
                    let mut current_item_id: Box<str> = new_parent.clone();
                    loop {
                        match self.children.get(current_item_id.as_ref()) {
                            None => return Err(Box::new(AuError::NoSuchKey(current_item_id))),
                            Some(current_item_ref) => {
                                let current_item = current_item_ref.clone();
                                match current_item.parent.as_ref() {
                                    None => break,
                                    Some(p) => {
                                        if p.as_ref().eq(id) {
                                            return Err(Box::new(AuError::InvalidOperation(new_parent.clone(), Box::from("has a cycle"))));
                                        }
                                        current_item_id = Box::from(p.as_ref())
                                    }
                                }
                            }
                        };
                    }

                    new_item.parent = Some(Rc::from(new_parent.as_ref()));
                    doc.put(
                        &item_node,
                        DOC_ITEM_PARENT_NODE,
                        ScalarValue::Str(SmolStr::from(new_parent.clone())),
                    )?;
                }
                ItemUpdate::Rank(_) => {}
                ItemUpdate::Class(_) => {}
                ItemUpdate::Content(_, _) => {}
            }
        }

        self.children.insert(Box::from(id), Rc::from(new_item));
        Ok(self)
    }

    pub fn has_children(&self, parent: Option<&str>) -> bool {
        for (_, v) in self.children.iter() {
            match (parent, &v.parent) {
                (Some(pa), Some(pb)) => {
                    if pa == pb.as_ref() {
                        return true;
                    }
                }
                (None, None) => return true,
                _ => (),
            }
        }
        return false;
    }

    pub fn list_children(&self, parent: Option<&str>) -> Vec<Rc<Item>> {
        let mut out: Vec<Rc<Item>> = Vec::new();
        for (_, v) in self.children.iter() {
            match (parent, &v.parent) {
                (Some(pa), Some(pb)) => {
                    if pa == pb.as_ref() {
                        out.push(v.clone())
                    }
                }
                (None, None) => out.push(v.clone()),
                _ => (),
            }
        }
        out.sort_by(|a, b| {
            if a.rank > b.rank || (a.rank == b.rank && a.at.lt(&b.at)) {
                Ordering::Less
            } else if a.rank > b.rank || (a.rank == b.rank && a.at.gt(&b.at)) {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });

        return out;
    }
}

fn decode_item_inner(source: &Automerge, item_node: &automerge::ObjId, k: &str) -> Result<Option<Item>, Box<dyn std::error::Error>> {
    let mut new_item = Item::default();
    new_item.id = Rc::from(k);

    // required fields
    new_item.at = decode_timestamp(&source, &item_node, DOC_ITEM_AT_NODE)?
        .map_or_else(|| Err(Box::new(AuError::NoSuchKey(Box::from(DOC_ITEM_AT_NODE)))), Ok)?;
    new_item.content = Rc::from(
        decode_content(&source, &item_node, DOC_ITEM_CONTENT_NODE)?
            .map_or_else(|| Err(Box::new(AuError::NoSuchKey(Box::from(DOC_ITEM_CONTENT_NODE)))), Ok)?,
    );
    new_item.content_type = Rc::from(
        decode_string(&source, &item_node, DOC_ITEM_CONTENT_TYPE_NODE)?
            .map_or_else(|| Err(Box::new(AuError::NoSuchKey(Box::from(DOC_ITEM_CONTENT_TYPE_NODE)))), Ok)?,
    );

    // optional fields
    new_item.parent = decode_string(&source, &item_node, DOC_ITEM_PARENT_NODE)?.map(|x| Rc::from(x));
    new_item.class = decode_string(&source, &item_node, DOC_ITEM_CLASS_NODE)?.map(|x| Rc::from(x));
    new_item.rank = decode_i64(&source, &item_node, DOC_ITEM_RANK_NODE)?.unwrap_or(0);

    return Ok(Some(new_item));
}

pub fn decode_item(source: &Automerge, items_node: &automerge::ObjId, k: &str) -> Result<Option<Item>, Box<dyn std::error::Error>> {
    let item_node = match source.get(items_node, k) {
        Ok(Some((Value::Object(ObjType::Map), n))) => n,
        Ok(Some(_)) => return Err(Box::new(AuError::IncorrectType(Box::from(k), Box::from("map")))),
        Ok(None) => return Ok(None),
        Err(e) => return Err(Box::new(e)),
    };
    match decode_item_inner(source, &item_node, k) {
        Ok(v) => Ok(v),
        Err(e) => Err(Box::new(AuError::NestedError(Box::from(k), e))),
    }
}

fn find_items_node(doc: &Automerge) -> Result<automerge::ObjId, Box<dyn std::error::Error>> {
    match doc.get(automerge::ROOT, DOC_ITEMS_NODE) {
        Ok(Some((Value::Object(ObjType::Map), n))) => Ok(n),
        Ok(Some(_)) => return Err(Box::new(AuError::IncorrectType(Box::from(DOC_ITEMS_NODE), Box::from("map")))),
        Ok(None) => return Err(Box::new(AuError::NoSuchKey(Box::from(DOC_ITEMS_NODE)))),
        Err(e) => return Err(Box::new(AuError::NestedError(Box::from(DOC_ITEMS_NODE), Box::new(e)))),
    }
}

pub fn decode_project(source: &Automerge) -> Result<Project, Box<dyn std::error::Error>> {
    let items_node = find_items_node(source)?;
    let mut out: HashMap<Box<str>, Rc<Item>> = HashMap::new();
    let keys = source.keys(&items_node);
    for k in keys {
        let new_item =
            decode_item(source, &items_node, k.as_str()).map_err(|e| Box::new(AuError::NestedError(Box::from(DOC_ITEMS_NODE), e)))?;
        if new_item.is_none() {
            return Err(Box::new(AuError::NoSuchKey(Box::from(k))));
        }
        out.insert(Box::from(k), Rc::from(new_item.unwrap()));
    }

    return Ok(Project { children: out });
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use automerge::transaction::Transactable;
    use automerge::{AutoCommit, ObjType, ScalarValue};

    use crate::item::{decode_item, decode_project, Item, ItemUpdate, Project};

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
        assert_eq!(item.id.as_ref(), "some-id");
        assert_eq!(item.at.year(), 2024);
        assert_eq!(item.content_type.as_ref(), "text/markdown");
        assert_eq!(item.content.len(), 9);
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
        assert_eq!(item.id.as_ref(), "some-id");
        assert_eq!(item.at.year(), 2024);
        assert_eq!(item.content_type.as_ref(), "text/markdown");
        assert_eq!(item.content.len(), 0);
        assert_eq!(item.rank, 42);
        assert!(item.class.is_some());
        assert_eq!(item.class.unwrap().as_ref(), "todo");
        assert_eq!(item.parent.unwrap().as_ref(), "other-id");
    }

    #[test]
    fn test_decode_project_empty() {
        let mut doc = AutoCommit::new();
        doc.put_object(automerge::ROOT, "items", ObjType::Map).unwrap();
        let mut project = decode_project(doc.document()).unwrap();
        assert_eq!(project.children.len(), 0);
        assert!(!project.has_children(None));
        assert_eq!(
            project.without_item("thing", &mut doc).expect_err("").to_string(),
            "'thing': no such key"
        );
        assert_eq!(project.children.len(), 0);
        assert!(!project.has_children(None));
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
        let mut project = Project::default();
        let mut item = Item::default();
        item.id = Rc::from("some-id");
        item.content_type = Rc::from("text/markdown");
        item.content = Rc::from("blah blah".as_bytes());
        project.with_item(&item, &mut doc).unwrap();
        let project = decode_project(doc.document()).unwrap();
        assert_eq!(project.children.len(), 1);
        assert!(project.children.get("some-id").is_some());
        assert_eq!(project.list_children(None).len(), 1);
        assert!(project.has_children(None));
        assert_eq!(project.list_children(Some("foo")).len(), 0);
        assert!(!project.has_children(Some("foo")));
    }

    #[test]
    fn test_project_with_item_tree() {
        let mut doc = AutoCommit::new();
        let mut project = Project::default();
        let mut item_a = Item::default();
        item_a.id = Rc::from("item-a");
        let mut item_b = Item::default();
        item_b.id = Rc::from("item-b");
        item_b.parent = Some(Rc::from("item-a"));
        let mut item_c = Item::default();
        item_c.id = Rc::from("item-c");
        item_c.parent = Some(Rc::from("item-b"));
        project
            .with_item(&item_a, &mut doc)
            .unwrap()
            .with_item(&item_b, &mut doc)
            .unwrap()
            .with_item(&item_c, &mut doc)
            .unwrap();

        assert_eq!(project.children.len(), 3);
        assert_eq!(project.list_children(None).len(), 1);
        assert_eq!(project.list_children(Some("item-a")).len(), 1);
        assert_eq!(project.list_children(Some("item-b")).len(), 1);
        assert_eq!(project.list_children(Some("item-c")).len(), 0);

        project.without_item("item-c", &mut doc).unwrap();

        assert_eq!(project.children.len(), 2);
        assert_eq!(project.list_children(None).len(), 1);
        assert_eq!(project.list_children(Some("item-a")).len(), 1);
        assert_eq!(project.list_children(Some("item-b")).len(), 0);

        project.with_updated_item("item-b", &[ItemUpdate::Parent(None)], &mut doc).unwrap();

        assert_eq!(project.children.len(), 2);
        assert_eq!(project.list_children(None).len(), 2);
        assert_eq!(project.list_children(Some("item-a")).len(), 0);
        assert_eq!(project.list_children(Some("item-b")).len(), 0);

        project
            .with_updated_item("item-a", &[ItemUpdate::Parent(Some(Box::from("item-b")))], &mut doc)
            .unwrap();

        assert_eq!(project.children.len(), 2);
        assert_eq!(project.list_children(None).len(), 1);
        assert_eq!(project.list_children(Some("item-a")).len(), 0);
        assert_eq!(project.list_children(Some("item-b")).len(), 1);

        // make sure a new document agrees

        project = decode_project(doc.document()).unwrap();

        assert_eq!(project.children.len(), 2);
        assert_eq!(project.list_children(None).len(), 1);
        assert_eq!(project.list_children(Some("item-a")).len(), 0);
        assert_eq!(project.list_children(Some("item-b")).len(), 1);
    }
}
