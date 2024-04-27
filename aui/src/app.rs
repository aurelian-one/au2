
/*

main screen shows a tree centered on a particular item
- up and down moves between siblings in rank order
- left moves up to the parent
- right moves into the node to the first child
- enter opens the item and shows all text to scroll through if it is text
- escape leaves
- shift enter adds a new item below the current
- alt enter adds a new item into the current at the bottom

 */

use std::rc::Rc;
use automerge::AutoCommit;
use ratatui::widgets::ListState;
use au::item::{Item, Project};

pub struct TreeContext {
    pub parents: Vec<Rc<Item>>,
    pub children: Vec<Rc<Item>>,
    pub list_state: ListState,
}

pub enum Mode {
    // Tree mode is the main view of the hierarchy
    Tree(TreeContext),
    // Detail mode is viewing the current item in detail
    Detail(Box<str>),
}

pub struct App {
    pub doc: automerge::AutoCommit,
    pub project: Project,
    pub mode: Mode,
}

impl App {
    pub fn new() -> App {
        App {
            doc: AutoCommit::new(),
            project: Project::default(),
            mode: Mode::Tree(TreeContext{
                parents: vec![],
                children: vec![],
                list_state: Default::default(),
            }),
        }
    }
}