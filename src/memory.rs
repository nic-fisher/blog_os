use x86_64::{
    structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PhysFrame, Size4KiB}, PhysAddr, VirtAddr
};
use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    // Create a FrameAllocator from the passed memory map.
    //
    // This function is unsafe because the caller must guarantee that the passed
    // memory map is valid. The main requirement is that all frames that are marked
    // as `USABLE` in it are really unused.

    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0
        }
    }

    // Returns an iterator over the usable frames specified in the memory map
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);

        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());

        // transform to an iterator of frame start address
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

// Initialize a new OffsetPageTable.
// This function is unsafe because the caller must guarantee that the
// passed physical_memory_offset is correct and the complete physical
// memory must be mapped in the virtual address space starting at the
// address physical_memory_offset. This means that for example physical
// address 0x5000 can be accessed through virtual address phys_offset + 0x5000.
// This mapping is required because the mapper needs to access page tables, which
// are not mapped into the virtual space by default.
// Also, the passed level_4_table used in the OffsetPage must point to the level
// 4 page table of a valid page table hierarchy. Otherwise this function might
// break memory safety, e.g. by writing to an illegal memory location.
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

// Returns a mutable reference to the active level 4 table.
//
// This function is unsafe because the caller must guarantee that the
// complete physical memory is mapped to virtual memory at the passed
// `physical_memory_offset`. Also, this function must be only called once
// to avoid aliasing `&mut` references (which is undefined behaviour).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    // Read the physical frame of the active level 4 table from the CR3 register
    let (level_4_table_frame, _) = Cr3::read();

    // Take the physical start address, convert it to a u64, and add it to physical_memory_offset to get
    // the virtual address where the page table frame is mapped.
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();

    // Convert the virtual address to a *mut PageTable raw pointer through the as_mut_ptr method and then
    // unsafely creates a &mut PageTable reference.
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    // Returns the frame that contains the given physical address.
    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    // PRESENT flag is required & WRITEABLE ensures we can write to the page.
    let flags = Flags::PRESENT | Flags::WRITABLE;


    let map_to_result = unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)
    };

    map_to_result.expect("map_to failed").flush();
}