use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode<T> {
    pub val: T,
    pub left: Option<Rc<RefCell<TreeNode<T>>>>,
    pub right: Option<Rc<RefCell<TreeNode<T>>>>,
}

impl<T> TreeNode<T> {
    pub fn new(val: T) -> Self {
        TreeNode {
            val,
            left: None,
            right: None,
        }
    }

    pub fn insert_left(&mut self, val: T) {
        self.left = Some(Rc::new(RefCell::new(TreeNode::new(val))));
    }

    pub fn insert_right(&mut self, val: T) {
        self.right = Some(Rc::new(RefCell::new(TreeNode::new(val))));
    }
}

