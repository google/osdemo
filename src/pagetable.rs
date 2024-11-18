// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use aarch64_paging::{
    paging::{
        Attributes, Constraints, MemoryRegion, PageTable, PhysicalAddress, Translation,
        TranslationRegime, VaRange, VirtualAddress,
    },
    MapError, Mapping,
};
use buddy_system_allocator::Heap;
use core::{
    alloc::Layout,
    ptr::{self, NonNull},
};

const ASID: usize = 1;
const ROOT_LEVEL: usize = 1;

pub const DEVICE_ATTRIBUTES: Attributes = Attributes::VALID
    .union(Attributes::ATTRIBUTE_INDEX_0)
    .union(Attributes::ACCESSED)
    .union(Attributes::UXN);
pub const MEMORY_ATTRIBUTES: Attributes = Attributes::VALID
    .union(Attributes::ATTRIBUTE_INDEX_1)
    .union(Attributes::INNER_SHAREABLE)
    .union(Attributes::ACCESSED)
    .union(Attributes::NON_GLOBAL);

#[derive(Debug)]
struct IdTranslation {
    page_allocator: Heap<32>,
}

impl IdTranslation {
    fn virtual_to_physical(va: VirtualAddress) -> PhysicalAddress {
        PhysicalAddress(va.0)
    }
}

impl Translation for IdTranslation {
    fn allocate_table(&mut self) -> (NonNull<PageTable>, PhysicalAddress) {
        let layout = Layout::new::<PageTable>();
        let pointer = self
            .page_allocator
            .alloc(layout)
            .expect("Failed to allocate page for pagetable");
        // SAFETY: The allocator has just given us a new allocation so it must be valid and
        // unaliased.
        unsafe {
            ptr::write_bytes(pointer.as_ptr(), 0, layout.size());
        }
        let table = pointer.cast();

        // Physical address is the same as the virtual address because we are using identity mapping
        // everywhere.
        (table, PhysicalAddress(table.as_ptr() as usize))
    }

    unsafe fn deallocate_table(&mut self, page_table: NonNull<PageTable>) {
        let layout = Layout::new::<PageTable>();
        self.page_allocator.dealloc(page_table.cast(), layout);
    }

    fn physical_to_virtual(&self, pa: PhysicalAddress) -> NonNull<PageTable> {
        NonNull::new(pa.0 as *mut PageTable).expect("Got physical address 0 for pagetable")
    }
}

/// Manages a page table using identity mapping.
pub struct IdMap {
    mapping: Mapping<IdTranslation>,
}

impl IdMap {
    /// Creates a new `IdMap` using the given page allocator.
    pub fn new(page_allocator: Heap<32>) -> Self {
        Self {
            mapping: Mapping::new(
                IdTranslation { page_allocator },
                ASID,
                ROOT_LEVEL,
                TranslationRegime::El1And0,
                VaRange::Lower,
            ),
        }
    }

    /// Returns the size in bytes of the virtual address space which can be mapped in this page
    /// table.
    pub fn size(&self) -> usize {
        self.mapping.size()
    }

    /// Identity-maps the given range of pages with the given flags.
    pub fn map_range(&mut self, range: &MemoryRegion, flags: Attributes) -> Result<(), MapError> {
        let pa = IdTranslation::virtual_to_physical(range.start());
        self.mapping
            .map_range(range, pa, flags, Constraints::empty())
    }

    /// Activates the page table by setting `TTBR0_EL1` to point to it.
    ///
    /// Panics if the `IdMap` has already been activated.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the page table doesn't unmap any memory which the program is
    /// using. The page table must not be dropped as long as its mappings are required, as it will
    /// automatically be deactivated when it is dropped.
    pub unsafe fn activate(&mut self) {
        // SAFETY: The caller has ensured that the page table doesn't unmap any memory and is held
        // for long enough. Mappings are unique because it uses identity mapping, so it won't
        // introduce any aliases.
        unsafe {
            self.mapping.activate();
        }
    }
}
