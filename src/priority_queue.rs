use ahash::RandomState;
use std::collections::HashMap;
use std::hash::Hash;

/// A specialized priority queue for HeavyKeeper that maintains top-k items by count
pub(crate) struct TopKQueue<T> {
    items: HashMap<T, (u64, usize), RandomState>, // item -> (count, heap_index)
    heap: Vec<(u64, usize, usize)>,               // (count, sequence, item_index)
    item_store: Vec<T>,                           // Store actual items here
    free_slots: Vec<usize>,                       // Track free slots in item_store
    capacity: usize,
    sequence: usize,
}

impl<T: Ord + Clone + Hash + PartialEq> TopKQueue<T> {
    pub(crate) fn with_capacity_and_hasher(capacity: usize, hasher: RandomState) -> Self {
        Self {
            items: HashMap::with_capacity_and_hasher(capacity, hasher),
            heap: Vec::with_capacity(capacity + 1),
            item_store: Vec::with_capacity(capacity),
            free_slots: Vec::with_capacity(capacity),
            capacity,
            sequence: 0,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, RandomState::new())
    }

    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn get(&self, item: &T) -> Option<u64> {
        self.items.get(item).map(|(count, _)| *count)
    }

    pub(crate) fn min_count(&self) -> u64 {
        // If heap is empty, return 0
        // Otherwise return count from root node (index 0)
        self.heap.first().map(|(count, _, _)| *count).unwrap_or(0)
    }

    pub(crate) fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    pub(crate) fn upsert(&mut self, item: T, count: u64) {
        // Fast path: update existing item
        if let Some((old_count, pos)) = self.items.get_mut(&item) {
            if count == *old_count {
                return;
            }
            *old_count = count;

            // Update heap - no need to clone item
            let pos = *pos;
            let item_idx = self.heap[pos].2;
            self.heap[pos] = (count, self.heap[pos].1, item_idx);
            self.sift_down(pos);
            self.sift_up(pos);
            return;
        }

        // For new items, if we have space just add it
        if self.len() < self.capacity {
            let pos = self.heap.len();
            self.sequence += 1;

            // Store item once
            let item_idx = if let Some(idx) = self.free_slots.pop() {
                self.item_store[idx] = item.clone();
                idx
            } else {
                self.item_store.push(item.clone());
                self.item_store.len() - 1
            };

            self.heap.push((count, self.sequence, item_idx));
            self.items.insert(item, (count, pos));
            self.sift_up(pos);
            return;
        }

        // Queue is full - check if new count beats minimum
        if let Some(&(min_count, _, item_idx)) = self.heap.first() {
            if count > min_count {
                let old_item = &self.item_store[item_idx];
                self.items.remove(old_item);

                // Reuse the item slot
                self.item_store[item_idx] = item.clone();
                self.items.insert(item, (count, 0));
                self.sequence += 1;
                self.heap[0] = (count, self.sequence, item_idx);
                self.sift_down(0);
            }
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&T, u64)> {
        let mut items: Vec<_> = self.items.iter().map(|(k, v)| (k, v.0)).collect();
        // Sort by count descending, then by sequence ascending
        items.sort_unstable_by(|(k1, v1), (k2, v2)| {
            match v2.cmp(v1) {
                std::cmp::Ordering::Equal => {
                    // For equal counts, compare sequence numbers
                    let seq1 = self
                        .heap
                        .iter()
                        .find(|(_, _, item_idx)| &self.item_store[*item_idx] == *k1)
                        .map(|(_, s, _)| *s)
                        .unwrap_or(0);
                    let seq2 = self
                        .heap
                        .iter()
                        .find(|(_, _, item_idx)| &self.item_store[*item_idx] == *k2)
                        .map(|(_, s, _)| *s)
                        .unwrap_or(0);
                    seq1.cmp(&seq2) // Earlier insertions first
                }
                other => other,
            }
        });
        items.into_iter()
    }

    // Binary heap helper methods using Eytzinger layout (0-based indexing)
    fn parent(i: usize) -> usize {
        (i - 1) >> 1
    }
    fn left(i: usize) -> usize {
        2 * i + 1
    }
    fn right(i: usize) -> usize {
        2 * i + 2
    }

    fn sift_up(&mut self, mut pos: usize) {
        while pos > 0 {
            let parent = Self::parent(pos);
            if self.heap[parent].0 > self.heap[pos].0 {
                self.swap_nodes(parent, pos);
                pos = parent;
            } else {
                break;
            }
        }
    }

    fn sift_down(&mut self, mut pos: usize) {
        loop {
            let mut smallest = pos;
            let left = Self::left(pos);
            let right = Self::right(pos);

            if left < self.heap.len() && self.heap[left].0 < self.heap[smallest].0 {
                smallest = left;
            }
            if right < self.heap.len() && self.heap[right].0 < self.heap[smallest].0 {
                smallest = right;
            }

            if smallest == pos {
                break;
            }

            self.swap_nodes(pos, smallest);
            pos = smallest;
        }
    }

    fn swap_nodes(&mut self, i: usize, j: usize) {
        self.heap.swap(i, j);
        // Update indices in items map
        let (_, _, item_idx_i) = self.heap[i];
        let (_, _, item_idx_j) = self.heap[j];

        // Get references to the actual items
        let item_i = &self.item_store[item_idx_i];
        let item_j = &self.item_store[item_idx_j];

        // Update the positions in the items map
        if let Some((_, pos_i)) = self.items.get_mut(item_i) {
            *pos_i = i;
        }
        if let Some((_, pos_j)) = self.items.get_mut(item_j) {
            *pos_j = j;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insertion() {
        let mut queue = TopKQueue::with_capacity(2);
        queue.upsert("a", 1);
        queue.upsert("b", 2);

        let items: Vec<_> = queue.iter().collect();
        assert_eq!(items, vec![(&"b", 2), (&"a", 1)]);
    }

    #[test]
    fn test_update_existing() {
        let mut queue = TopKQueue::with_capacity_and_hasher(2, RandomState::new());
        queue.upsert("a", 1);
        queue.upsert("b", 2);
        queue.upsert("a", 3); // Update a's count

        let items: Vec<_> = queue.iter().collect();
        assert_eq!(items, vec![(&"a", 3), (&"b", 2)]);
    }

    #[test]
    fn test_heap_cleanup() {
        let mut queue = TopKQueue::with_capacity_and_hasher(2, RandomState::new());

        // Insert initial items
        queue.upsert("a", 1);
        queue.upsert("b", 2);

        // Update 'a' multiple times
        queue.upsert("a", 3);
        queue.upsert("a", 4);
        queue.upsert("a", 5);

        // Insert new item with higher count
        queue.upsert("c", 6);

        // Check heap size vs items size
        assert_eq!(queue.heap.len(), 2, "Expected 2 items");

        let items: Vec<_> = queue.iter().collect();
        assert_eq!(items, vec![(&"c", 6), (&"a", 5)]);
    }

    #[test]
    fn test_insertion_order() {
        let mut queue = TopKQueue::with_capacity_and_hasher(3, RandomState::new());

        // Insert items with same count in specific order
        queue.upsert("a", 1);
        queue.upsert("b", 1);
        queue.upsert("c", 1);

        let items: Vec<_> = queue.iter().collect();
        assert_eq!(items, vec![(&"a", 1), (&"b", 1), (&"c", 1)]);
    }

    #[test]
    fn test_heap_consistency() {
        let mut queue = TopKQueue::with_capacity_and_hasher(2, RandomState::new());

        // Fill queue
        queue.upsert("a", 1);
        queue.upsert("b", 2);

        // Update existing item multiple times
        for i in 3..10 {
            queue.upsert("a", i);
        }

        // Try to insert new item
        queue.upsert("c", 5);

        // Verify min_count is accurate
        assert_eq!(queue.min_count(), 5);
    }

    #[test]
    fn test_capacity_overflow() {
        let mut queue = TopKQueue::with_capacity_and_hasher(2, RandomState::new());

        // Insert more items than capacity
        queue.upsert("a", 1);
        queue.upsert("b", 2);
        queue.upsert("c", 3);
        queue.upsert("d", 4);
        queue.upsert("e", 5);

        assert_eq!(queue.len(), 2, "Queue should maintain capacity");

        let items: Vec<_> = queue.iter().collect();
        assert_eq!(items, vec![(&"e", 5), (&"d", 4)]);
    }

    #[test]
    fn test_repeated_updates() {
        let mut queue = TopKQueue::with_capacity_and_hasher(2, RandomState::new());

        // Insert and update same item repeatedly
        for i in 1..100 {
            queue.upsert("a", i);
        }

        queue.upsert("b", 50);

        assert_eq!(queue.len(), 2);

        let items: Vec<_> = queue.iter().collect();
        assert_eq!(items, vec![(&"a", 99), (&"b", 50)]);
    }

    #[test]
    fn test_heap_property() {
        let mut queue = TopKQueue::with_capacity_and_hasher(10, RandomState::new());

        // Insert in reverse order to test heap maintenance
        for i in (0..=10).rev() {
            queue.upsert(format!("item{}", i), i as u64);
        }

        // Verify heap property: parent should be <= children for min-heap
        for i in 1..queue.heap.len() {
            let parent_idx = TopKQueue::<String>::parent(i);
            if parent_idx > 0 {
                // Skip root's parent
                assert!(queue.heap[parent_idx].0 <= queue.heap[i].0,
                    "Heap property violated: parent count {} at index {} is greater than child count {} at index {}", 
                    queue.heap[parent_idx].0, parent_idx, queue.heap[i].0, i);
            }
        }

        // Verify items are stored in descending order (highest counts first)
        let items: Vec<_> = queue.iter().collect();
        for i in 0..items.len() - 1 {
            assert!(
                items[i].1 >= items[i + 1].1,
                "Items not properly ordered by count: {} before {}",
                items[i].1,
                items[i + 1].1
            );
        }
    }
}
