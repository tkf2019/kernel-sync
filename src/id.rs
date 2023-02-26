use alloc::vec::Vec;

pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    pub fn new(current: usize) -> Self {
        Self {
            current,
            recycled: Vec::new(),
        }
    }
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            assert_ne!(self.current, usize::MAX);
            self.current - 1
        }
    }

    pub fn dealloc(&mut self, id: usize) {
        self.recycled.push(id);
    }
}
