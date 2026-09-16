//! Real-time signal processing primitives shared by standalone and plugin hosts.

mod convolver;
mod engine;

pub use convolver::*;
pub use engine::*;

#[cfg(test)]
mod test_alloc {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! { static TRACKING: Cell<bool> = const { Cell::new(false) }; static OPERATIONS: Cell<usize> = const { Cell::new(0) }; }
    pub struct CountingAllocator;
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            TRACKING.with(|tracking| {
                if tracking.get() {
                    OPERATIONS.with(|count| count.set(count.get() + 1));
                }
            });
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            TRACKING.with(|tracking| {
                if tracking.get() {
                    OPERATIONS.with(|count| count.set(count.get() + 1));
                }
            });
            unsafe { System.dealloc(pointer, layout) }
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            TRACKING.with(|tracking| {
                if tracking.get() {
                    OPERATIONS.with(|count| count.set(count.get() + 1));
                }
            });
            unsafe { System.realloc(pointer, layout, size) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;
    pub fn start() {
        OPERATIONS.with(|count| count.set(0));
        TRACKING.with(|tracking| tracking.set(true));
    }
    pub fn stop() -> usize {
        TRACKING.with(|tracking| tracking.set(false));
        OPERATIONS.with(Cell::get)
    }
}
