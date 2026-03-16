// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{exceptions::current_el, platform::PlatformImpl};
use aarch64_paging::{
    MapError, Mapping,
    descriptor::{
        El1Attributes, El23Attributes, PagingAttributes, PhysicalAddress, VirtualAddress,
    },
    paging::{Constraints, El1And0, El2, MemoryRegion, PageTable, Translation, VaRange},
};
use aarch64_rt::initial_pagetable;
use buddy_system_allocator::Heap;
use core::{
    alloc::Layout,
    marker::PhantomData,
    ptr::{self, NonNull},
};
use spin::Once;

const ASID: usize = 0;
const ROOT_LEVEL: usize = 1;

pub const EL1_DEVICE_ATTRIBUTES: El1Attributes = El1Attributes::VALID
    .union(El1Attributes::ATTRIBUTE_INDEX_0)
    .union(El1Attributes::ACCESSED)
    .union(El1Attributes::UXN);
pub const EL1_MEMORY_ATTRIBUTES: El1Attributes = El1Attributes::VALID
    .union(El1Attributes::ATTRIBUTE_INDEX_1)
    .union(El1Attributes::INNER_SHAREABLE)
    .union(El1Attributes::ACCESSED)
    .union(El1Attributes::NON_GLOBAL);
const EL2_DEVICE_ATTRIBUTES: El23Attributes = El23Attributes::VALID
    .union(El23Attributes::ATTRIBUTE_INDEX_0)
    .union(El23Attributes::ACCESSED)
    .union(El23Attributes::XN);
const EL2_MEMORY_ATTRIBUTES: El23Attributes = El23Attributes::VALID
    .union(El23Attributes::ATTRIBUTE_INDEX_1)
    .union(El23Attributes::INNER_SHAREABLE)
    .union(El23Attributes::ACCESSED)
    .union(El23Attributes::NON_GLOBAL);

pub static PAGETABLE: Once<IdMap> = Once::new();

#[derive(Debug)]
pub struct IdTranslation<A: PagingAttributes> {
    page_allocator: Heap<32>,
    _attributes: PhantomData<A>,
}

impl<A: PagingAttributes> IdTranslation<A> {
    fn new(page_allocator: Heap<32>) -> Self {
        Self {
            page_allocator,
            _attributes: PhantomData,
        }
    }

    fn virtual_to_physical(va: VirtualAddress) -> PhysicalAddress {
        PhysicalAddress(va.0)
    }
}

impl<A: PagingAttributes> Translation<A> for IdTranslation<A> {
    fn allocate_table(&mut self) -> (NonNull<PageTable<A>>, PhysicalAddress) {
        let layout = Layout::new::<PageTable<A>>();
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

    unsafe fn deallocate_table(&mut self, page_table: NonNull<PageTable<A>>) {
        let layout = Layout::new::<PageTable<A>>();
        // SAFETY: Our caller promises that the page table was allocated by `allocate_table` and not
        // yet deallocated, and it won't be used after this.
        unsafe {
            self.page_allocator.dealloc(page_table.cast(), layout);
        }
    }

    fn physical_to_virtual(&self, pa: PhysicalAddress) -> NonNull<PageTable<A>> {
        NonNull::new(pa.0 as *mut PageTable<A>).expect("Got physical address 0 for pagetable")
    }
}

// SAFETY: An `&IdTranslation` only allows looking up the mapping from a physical to virtual
// address, which is safe to do from any context.
unsafe impl<A: PagingAttributes> Sync for IdTranslation<A> {}

/// Manages a page table using identity mapping, at either EL1 or EL2.
#[derive(Debug)]
pub enum IdMap {
    El1 {
        mapping: Mapping<IdTranslation<El1Attributes>, El1And0>,
    },
    El2 {
        mapping: Mapping<IdTranslation<El23Attributes>, El2>,
    },
}

impl IdMap {
    /// Creates a new `IdMap` using the given page allocator.
    pub fn new(page_allocator: Heap<32>) -> Self {
        if current_el() == 2 {
            Self::El2 {
                mapping: Mapping::new(IdTranslation::new(page_allocator), ROOT_LEVEL, El2),
            }
        } else {
            Self::El1 {
                mapping: Mapping::with_asid_and_va_range(
                    IdTranslation::new(page_allocator),
                    ASID,
                    ROOT_LEVEL,
                    El1And0,
                    VaRange::Lower,
                ),
            }
        }
    }

    /// Returns the size in bytes of the virtual address space which can be mapped in this page
    /// table.
    pub fn size(&self) -> usize {
        match self {
            IdMap::El1 { mapping } => mapping.size(),
            IdMap::El2 { mapping } => mapping.size(),
        }
    }

    /// Identity-maps the given range of pages as normal memory.
    pub fn map_memory(&mut self, range: &MemoryRegion) -> Result<(), MapError> {
        match self {
            IdMap::El1 { mapping } => {
                let pa = IdTranslation::<El1Attributes>::virtual_to_physical(range.start());
                mapping.map_range(range, pa, EL1_MEMORY_ATTRIBUTES, Constraints::empty())
            }
            IdMap::El2 { mapping } => {
                let pa = IdTranslation::<El23Attributes>::virtual_to_physical(range.start());
                mapping.map_range(range, pa, EL2_MEMORY_ATTRIBUTES, Constraints::empty())
            }
        }
    }

    /// Identity-maps the given range of pages as device memory.
    pub fn map_device(&mut self, range: &MemoryRegion) -> Result<(), MapError> {
        match self {
            IdMap::El1 { mapping } => {
                let pa = IdTranslation::<El1Attributes>::virtual_to_physical(range.start());
                mapping.map_range(range, pa, EL1_DEVICE_ATTRIBUTES, Constraints::empty())
            }
            IdMap::El2 { mapping } => {
                let pa = IdTranslation::<El23Attributes>::virtual_to_physical(range.start());
                mapping.map_range(range, pa, EL2_DEVICE_ATTRIBUTES, Constraints::empty())
            }
        }
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
    pub unsafe fn activate(&self) {
        // SAFETY: The caller has ensured that the page table doesn't unmap any memory and is held
        // for long enough. Mappings are unique because it uses identity mapping, so it won't
        // introduce any aliases.
        unsafe {
            match self {
                IdMap::El1 { mapping } => {
                    mapping.activate();
                }
                IdMap::El2 { mapping } => {
                    mapping.activate();
                }
            }
        }
    }

    /// Activates the page table on a secondary CPU core by setting `TTBR0_EL1` to point to it.
    ///
    /// Panics if `IdMap` has not already been activated on the primary core.
    ///
    /// The page table must not be dropped as long as it is active on the secondary core. The static
    /// lifetime ensures this.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the page table doesn't unmap any memory which the program is
    /// using.
    pub unsafe fn activate_secondary(&'static self) {
        match self {
            IdMap::El1 { mapping } => {
                assert!(mapping.active());
            }
            IdMap::El2 { mapping } => {
                assert!(mapping.active());
            }
        }
        // SAFETY: Our caller promised that the page table doesn't unmapping anything which the
        // program needs. The static lifetime of &self ensures that the page table isn't dropped.
        unsafe {
            self.activate();
        }
    }
}

// The initial hardcoded page table used before the Rust code starts and activates the main page
// table.
initial_pagetable!(PlatformImpl::initial_idmap());
