struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    /// Creates a new, empty stack.
    pub fn new() -> Self {
        Stack { items: Vec::new() }
    }

    /// Adds an item to the top of the stack.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// Removes and returns the top item from the stack.
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    /// Returns the number of items in the stack.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

