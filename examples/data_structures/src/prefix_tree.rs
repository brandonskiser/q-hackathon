use std::collections::HashMap;

/// A node in the prefix tree
struct PrefixTreeNode {
    // Whether this node represents the end of a word
    is_end_of_word: bool,
    // The children of this node, keyed by the character
    children: HashMap<char, Box<PrefixTreeNode>>,
}

impl PrefixTreeNode {
    fn new() -> Self {
        PrefixTreeNode {
            is_end_of_word: false,
            children: HashMap::new(),
        }
    }
}

/// The prefix tree data structure
struct PrefixTree {
    root: PrefixTreeNode,
}

impl PrefixTree {
    /// Creates a new, empty prefix tree
    fn new() -> Self {
        PrefixTree {
            root: PrefixTreeNode::new(),
        }
    }

    /// Inserts a word into the prefix tree
    fn insert(&mut self, word: &str) {
        let mut current_node = &mut self.root;
        for c in word.chars() {
            current_node
                .children
                .entry(c)
                .or_insert_with(PrefixTreeNode::new);
            current_node = current_node.children.get_mut(&c).unwrap();
        }
        current_node.is_end_of_word = true;
    }

    /// Searches for a word in the prefix tree
    fn search(&self, word: &str) -> bool {
        let mut current_node = &self.root;
        for c in word.chars() {
            if let Some(next_node) = current_node.children.get(&c) {
                current_node = next_node;
            } else {
                return false;
            }
        }
        current_node.is_end_of_word
    }

    /// Starts a search with a prefix and returns all words that start with that prefix
    fn starts_with(&self, prefix: &str) -> Vec<String> {
        let mut current_node = &self.root;
        let mut result = Vec::new();
        let mut current_word = String::new();

        for c in prefix.chars() {
            if let Some(next_node) = current_node.children.get(&c) {
                current_word.push(c);
                current_node = next_node;
            } else {
                return result;
            }
        }

        self.dfs(current_node, &mut current_word, &mut result);
        result
    }

    /// Depth-first search to find all words that start with the given prefix
    fn dfs(&self, node: &PrefixTreeNode, current_word: &mut String, result: &mut Vec<String>) {
        if node.is_end_of_word {
            result.push(current_word.clone());
        }

        for (c, child) in node.children.iter() {
            current_word.push(*c);
            self.dfs(child, current_word, result);
            current_word.pop();
        }
    }
}

