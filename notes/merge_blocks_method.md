# Understanding the `merge_blocks` Method in SPIMI

This document captures the complete design, code implementations, conceptual foundations, Rust-specific idioms, and a step-by-step variable-by-variable walkthrough for the `merge_blocks` method in `src/inverted_index.rs`.

---

## 1. High-Level Overview: What is `merge_blocks`?

In **SPIMI (Single-Pass In-Memory Indexing)**:
1. Documents are added to an in-memory inverted index (`self.index`).
2. When memory hits a limit (`block_size_limit`), `write_block()` sorts the terms alphabetically and writes them to disk as intermediate block files (`cache/block_{i}.json`).
3. Once all documents are processed, **`merge_blocks()`** performs an **N-way merge** across all block files to produce a single, unified inverted index file (`cache/merged_index.bincode`).

---

## 2. Implementations

### Implementation 1: Idiomatic SPIMI N-Way Merge (Using `BinaryHeap`)

This is the standard algorithm matching the 4 commented steps in `src/inverted_index.rs`:

```rust
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;
use std::io::{BufReader, BufWriter};

fn merge_blocks(&self) {
    // 1. Open all generated block files from the `cache/` directory.
    let mut block_iters = Vec::new();

    for i in 0..self.block_counter {
        let block_path = format!("cache/block_{}.json", i);
        if let Ok(file) = File::open(&block_path) {
            let reader = BufReader::new(file);
            let entries: Vec<(String, Vec<u32>)> = bincode::deserialize_from(reader)
                .expect("Failed to deserialize intermediate block file");
            block_iters.push(entries.into_iter());
        }
    }

    // If no blocks were written, return early
    if block_iters.is_empty() {
        return;
    }

    // 2. Perform an N-way merge across all block streams using a Min-Heap / BinaryHeap.
    // In Rust, BinaryHeap is a Max-Heap by default, so std::cmp::Reverse is used for Min-Heap ordering.
    // Heap stores: Reverse((term, posting_list, block_index))
    let mut min_heap: BinaryHeap<Reverse<(String, Vec<u32>, usize)>> = BinaryHeap::new();

    // Seed the heap with the first (smallest) term from each block stream
    for (block_idx, iter) in block_iters.iter_mut().enumerate() {
        if let Some((term, postings)) = iter.next() {
            min_heap.push(Reverse((term, postings, block_idx)));
        }
    }

    let mut merged_index: HashMap<String, Vec<u32>> = HashMap::new();
    let mut current_term: Option<String> = None;
    let mut current_postings: Vec<u32> = Vec::new();

    // 3. For identical terms across different blocks, merge and concatenate their posting lists.
    while let Some(Reverse((term, postings, block_idx))) = min_heap.pop() {
        if let Some(ref cur) = current_term {
            if *cur == term {
                // Same term found in another block: append posting list
                current_postings.extend(postings);
            } else {
                // Reached a new term: commit previous term into the merged index
                merged_index.insert(current_term.take().unwrap(), current_postings);
                current_term = Some(term);
                current_postings = postings;
            }
        } else {
            current_term = Some(term);
            current_postings = postings;
        }

        // Advance the iterator for the block we just popped from and push the next entry
        if let Some((next_term, next_postings)) = block_iters[block_idx].next() {
            min_heap.push(Reverse((next_term, next_postings, block_idx)));
        }
    }

    // Commit the final term
    if let Some(term) = current_term {
        merged_index.insert(term, current_postings);
    }

    // 4. Write the final merged dictionary to a single file (cache/merged_index.bincode).
    let output_file = File::create("cache/merged_index.bincode")
        .expect("Failed to create merged index file");
    let writer = BufWriter::new(output_file);
    bincode::serialize_into(writer, &merged_index)
        .expect("Failed to serialize final merged index");
}
```

---

### Implementation 2: Simplified Direct Merge (In-Memory Accumulation)

If you don't strictly require streaming heap merges and all block data fits comfortably in RAM:

```rust
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

fn merge_blocks(&self) {
    let mut merged_index: HashMap<String, Vec<u32>> = HashMap::new();

    // 1 & 2. Open each block file and merge entries
    for i in 0..self.block_counter {
        let block_path = format!("cache/block_{}.json", i);
        if let Ok(file) = File::open(&block_path) {
            let reader = BufReader::new(file);
            let entries: Vec<(String, Vec<u32>)> = bincode::deserialize_from(reader)
                .expect("Failed to deserialize block");

            // 3. Concatenate posting lists for matching terms
            for (term, postings) in entries {
                merged_index
                    .entry(term)
                    .or_default()
                    .extend(postings);
            }
        }
    }

    // 4. Serialize to cache/merged_index.bincode
    let output_file = File::create("cache/merged_index.bincode")
        .expect("Failed to create merged index file");
    let writer = BufWriter::new(output_file);
    bincode::serialize_into(writer, &merged_index)
        .expect("Failed to serialize final merged index");
}
```

---

## 3. Deep Dive into Implementation 1 (The Heap-Based Merge)

### The First Half: Loading Blocks and Seeding the Heap

1. **`block_iters: Vec<std::vec::IntoIter<(String, Vec<u32>)>>`**:
   - We loop from `0..self.block_counter` to locate each `cache/block_{i}.json` written during the indexing phase.
   - `BufReader` wraps each file to read in large buffer chunks from the filesystem, minimizing system calls.
   - `bincode::deserialize_from` parses the serialized vector into `entries: Vec<(String, Vec<u32>)>`.
   - Calling `entries.into_iter()` transforms each block into a moving iterator. We store all block iterators inside `block_iters`.
2. **Early Return**:
   - `if block_iters.is_empty() { return; }` guards against running merge operations when zero documents or blocks exist.
3. **Why Min-Heap instead of Max-Heap?**
   - Rust's `std::collections::BinaryHeap` is a **Max-Heap** by default (it pops the largest value first).
   - Alphabetically, `"zebra"` > `"apple"`. A Max-Heap would pop words in descending Z → A order.
   - Wrapping elements in `std::cmp::Reverse` inverts the comparison logic, turning it into a **Min-Heap** so it pops in standard **ascending A → Z order**.
4. **How does the Heap know what to compare?**
   - We push tuples: `Reverse((term, postings, block_idx))`.
   - In Rust, tuples implement `Ord` lexicographically by comparing elements from left to right:
     1. First compares `term` (`String`).
     2. Only if `term`s are identical does it compare `postings`.
     3. Only if both are identical does it compare `block_idx`.
   - Therefore, the heap is ordered directly by the **alphabetical sorting of the words**.
5. **Why is sorting alphabetically in each block so important?**
   - Because each individual block is already sorted from A to Z, the globally smallest remaining word across the entire corpus must reside at the current head of one of the blocks.
   - Once all instances of a word (e.g. `"apple"`) are popped from the heap, **that word will never appear again in any block**.

---

### The Second Half: The Loop Mechanics and Rust Idioms

1. **`while let Some(Reverse((term, postings, block_idx))) = min_heap.pop()`**:
   - `.pop()` retrieves and removes the smallest element from the heap.
   - If the heap is empty, `.pop()` returns `None`, terminating the loop.
   - Pattern matching extracts `term`, `postings`, and `block_idx`.
2. **`if let Some(ref cur) = current_term`**:
   - **What is `ref cur`?**
     - Matching `Some(cur)` would try to take ownership of (move) the `String` out of `current_term`.
     - `ref cur` tells Rust to borrow the inner `String` as an immutable reference (`&String`) without moving it. *(In modern Rust, `if let Some(cur) = &current_term` is equivalent).*
3. **`if *cur == term` (Term Collision across Blocks)**:
   - We dereference `cur` and compare it to `term`.
   - If equal, the exact same word appears in another block.
   - `current_postings.extend(postings)` appends document IDs to our running list.
   - Because blocks were generated sequentially, document IDs remain naturally sorted in ascending order.
4. **The Inner `else` (Term Transition)**:
   - If `*cur != term`, we have seen every instance of the old word across all blocks.
   - `merged_index.insert(current_term.take().unwrap(), current_postings);`:
     - `current_term.take()` extracts the `String` out of `current_term`, leaving `None` in its place.
     - `.unwrap()` gives the owned `String`, transferring ownership into `merged_index` without cloning.
     - `current_postings` is moved directly into `merged_index`.
   - `current_term = Some(term);`: Saves the newly popped term so future loop iterations can compare against it.
   - `current_postings = postings;`: Initializes the postings list for the new term.
5. **The Outer `else` (First Iteration)**:
   - Runs only when `current_term` is `None` (on iteration 1) to initialize the accumulator.
6. **Replenishing the Heap**:
   ```rust
   if let Some((next_term, next_postings)) = block_iters[block_idx].next() {
       min_heap.push(Reverse((next_term, next_postings, block_idx)));
   }
   ```
   - We consumed one entry from `block_idx`.
   - We pull the next item from that specific block's iterator.
   - If that block still has items, we push the new tuple into `min_heap`.
   - If that block is empty, we do nothing; that block is finished.
7. **Post-Loop Flush**:
   ```rust
   if let Some(term) = current_term {
       merged_index.insert(term, current_postings);
   }
   ```
   - When the heap empties, the loop ends, but the very last word processed remains in `current_term` and `current_postings`. This final block commits it to `merged_index`.

---

## 4. Concrete Walkthrough with Variable State Tracking

### Input Data

Suppose we have 2 blocks on disk:

* **Block 0** (`block_idx = 0`):
  1. `("apple", vec![1, 5])`
  2. `("cat", vec![3])`
* **Block 1** (`block_idx = 1`):
  1. `("apple", vec![9])`
  2. `("dog", vec![7, 8])`

### Initial Seeding (Before the Loop)

The seeding loop reads the first entry from each block into `min_heap`:

| Variable | State |
| :--- | :--- |
| `min_heap` | Contains: `Reverse(("apple", [1, 5], 0))` and `Reverse(("apple", [9], 1))` |
| `block_iters[0]` | Remaining: `[("cat", [3])]` |
| `block_iters[1]` | Remaining: `[("dog", [7, 8])]` |
| `current_term` | `None` |
| `current_postings` | `[]` |
| `merged_index` | `{}` |

---

### Iteration 1

1. **Pop from Heap**:
   - `min_heap.pop()` returns `Reverse(("apple", [1, 5], 0))`.
   - `term = "apple"`, `postings = [1, 5]`, `block_idx = 0`.
2. **Condition**:
   - `current_term` is `None`. Jumps to the **outer `else`**.
   - `current_term = Some("apple")`.
   - `current_postings = vec![1, 5]`.
3. **Refill from Block 0**:
   - `block_iters[0].next()` yields `("cat", [3])`.
   - Push `Reverse(("cat", [3], 0))` into `min_heap`.

**End of Iteration 1 State:**
| Variable | Value |
| :--- | :--- |
| `current_term` | `Some("apple")` |
| `current_postings` | `[1, 5]` |
| `merged_index` | `{}` |
| `min_heap` | Top: `Reverse(("apple", [9], 1))`<br>Next: `Reverse(("cat", [3], 0))` |

---

### Iteration 2

1. **Pop from Heap**:
   - `min_heap.pop()` returns `Reverse(("apple", [9], 1))` *(since `"apple"` < `"cat"`)*.
   - `term = "apple"`, `postings = [9]`, `block_idx = 1`.
2. **Condition**:
   - `current_term` is `Some("apple")` -> `cur` is `"apple"`.
   - `*cur == term` (`"apple" == "apple"`) is **TRUE**.
   - Hits the first `if` branch:
     ```rust
     current_postings.extend(postings); // appends [9]
     ```
   - `current_postings` becomes `[1, 5, 9]`.
3. **Refill from Block 1**:
   - `block_iters[1].next()` yields `("dog", [7, 8])`.
   - Push `Reverse(("dog", [7, 8], 1))` into `min_heap`.

**End of Iteration 2 State:**
| Variable | Value |
| :--- | :--- |
| `current_term` | `Some("apple")` |
| `current_postings` | `[1, 5, 9]` |
| `merged_index` | `{}` |
| `min_heap` | Top: `Reverse(("cat", [3], 0))`<br>Next: `Reverse(("dog", [7, 8], 1))` |

---

### Iteration 3

1. **Pop from Heap**:
   - `min_heap.pop()` returns `Reverse(("cat", [3], 0))` *(since `"cat"` < `"dog"`)*.
   - `term = "cat"`, `postings = [3]`, `block_idx = 0`.
2. **Condition**:
   - `current_term` is `Some("apple")` -> `cur` is `"apple"`.
   - `*cur == term` (`"apple" == "cat"`) is **FALSE**.
   - Hits the **inner `else`**:
     ```rust
     merged_index.insert(current_term.take().unwrap(), current_postings);
     // Inserts "apple" => [1, 5, 9]
     current_term = Some("cat");
     current_postings = vec![3];
     ```
3. **Refill from Block 0**:
   - `block_iters[0].next()` returns `None` (Block 0 is exhausted).
   - Nothing pushed to the heap.

**End of Iteration 3 State:**
| Variable | Value |
| :--- | :--- |
| `current_term` | `Some("cat")` |
| `current_postings` | `[3]` |
| `merged_index` | `{"apple": [1, 5, 9]}` |
| `min_heap` | Only element: `Reverse(("dog", [7, 8], 1))` |

---

### Iteration 4

1. **Pop from Heap**:
   - `min_heap.pop()` returns `Reverse(("dog", [7, 8], 1))`.
   - `term = "dog"`, `postings = [7, 8]`, `block_idx = 1`.
2. **Condition**:
   - `current_term` is `Some("cat")` -> `cur` is `"cat"`.
   - `*cur == term` (`"cat" == "dog"`) is **FALSE**.
   - Hits the **inner `else`**:
     ```rust
     merged_index.insert(current_term.take().unwrap(), current_postings);
     // Inserts "cat" => [3]
     current_term = Some("dog");
     current_postings = vec![7, 8];
     ```
3. **Refill from Block 1**:
   - `block_iters[1].next()` returns `None` (Block 1 is exhausted).
   - Nothing pushed to the heap.

**End of Iteration 4 State:**
| Variable | Value |
| :--- | :--- |
| `current_term` | `Some("dog")` |
| `current_postings` | `[7, 8]` |
| `merged_index` | `{"apple": [1, 5, 9], "cat": [3]}` |
| `min_heap` | **Empty** |

---

### Loop Termination & Final Flush

On the next iteration, `min_heap.pop()` returns `None`. The `while let` loop terminates.

At this point:
- `current_term` is `Some("dog")`.
- `current_postings` is `[7, 8]`.

Because no subsequent word arrived to trigger the inner `else`, `"dog"` is committed by the code after the loop:
```rust
if let Some(term) = current_term {
    merged_index.insert(term, current_postings);
}
```

### Final State of `merged_index`:
```json
{
  "apple": [1, 5, 9],
  "cat":   [3],
  "dog":   [7, 8]
}
```

---

## 5. Architectural Comparison: Implementation 1 vs. Implementation 2

| Metric / Aspect | Implementation 1 (Heap N-Way Merge) | Implementation 2 (Direct Hash Merge) |
| :--- | :--- | :--- |
| **Data Structures** | `BinaryHeap`, `std::cmp::Reverse`, Iterators | `HashMap`, `BufReader` |
| **How it Operates** | Merges sorted streams concurrently in alphabetical order. | Processes one block file at a time, inserting entries into a map. |
| **Time Complexity** | **$O(N \log K)$**, where $N$ is total terms across all blocks and $K$ is the number of blocks. | **$O(N)$** hash insertions and rehashing overhead. |
| **Memory Footprint** | Streaming model: only needs the active head of each block iterator in the heap. | Loads all blocks into a single large in-memory `HashMap`. |
| **Why SPIMI Uses It** | External merge sort: designed for datasets that exceed RAM capacity. | Simpler, but partially counteracts the memory savings of block splitting if the final map must reside in RAM. |
