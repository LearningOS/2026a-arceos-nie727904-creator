#![no_std]

use allocator::{
    AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator,
};
use core::alloc::Layout;
use core::ptr::NonNull;

/// A double-ended bump allocator.
///
/// Bytes grow upward; pages grow downward.
/// Pages are never reclaimed.
/// Byte allocations are reclaimed together once all are released.
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }

    fn align_down(value: usize, align: usize) -> usize {
        value & !(align - 1)
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        assert!(SIZE.is_power_of_two());

        let end = start.checked_add(size).expect("invalid memory region");

        self.start = start;
        self.end = Self::align_down(end, SIZE);
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;

        assert!(self.end > self.start);
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        // A linear bump allocator cannot merge arbitrary disjoint regions.
        // A contiguous extension is supported before page allocations.
        if start != self.end || self.p_pos != self.end {
            return Err(AllocError::NoMemory);
        }

        let new_end = start
            .checked_add(size)
            .ok_or(AllocError::NoMemory)?;

        let new_end = Self::align_down(new_end, SIZE);
        if new_end <= self.end {
            return Err(AllocError::NoMemory);
        }

        self.end = new_end;
        self.p_pos = new_end;
        Ok(())
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align = layout.align();

        let aligned = self
            .b_pos
            .checked_add(align - 1)
            .map(|addr| Self::align_down(addr, align))
            .ok_or(AllocError::NoMemory)?;

        let next = aligned
            .checked_add(layout.size().max(1))
            .ok_or(AllocError::NoMemory)?;

        if next > self.p_pos {
            return Err(AllocError::NoMemory);
        }

        let ptr = NonNull::new(aligned as *mut u8)
            .ok_or(AllocError::NoMemory)?;

        self.b_pos = next;
        self.count += 1;

        Ok(ptr)
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        if self.count > 0 {
            self.count -= 1;

            if self.count == 0 {
                self.b_pos = self.start;
            }
        }
    }

    fn total_bytes(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    fn used_bytes(&self) -> usize {
        self.b_pos.saturating_sub(self.start)
    }

    fn available_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> AllocResult<usize> {
        if num_pages == 0
            || !align_pow2.is_power_of_two()
            || align_pow2 < SIZE
        {
            return Err(AllocError::NoMemory);
        }

        let bytes = num_pages
            .checked_mul(SIZE)
            .ok_or(AllocError::NoMemory)?;

        let next = self
            .p_pos
            .checked_sub(bytes)
            .ok_or(AllocError::NoMemory)?;

        let next = Self::align_down(next, align_pow2);

        if next < self.b_pos {
            return Err(AllocError::NoMemory);
        }

        self.p_pos = next;

        Ok(next)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // By design, page allocations are not reclaimed.
    }

    fn total_pages(&self) -> usize {
        self.total_bytes() / SIZE
    }

    fn used_pages(&self) -> usize {
        self.end.saturating_sub(self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        self.available_bytes() / SIZE
    }
}
