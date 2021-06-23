use std::mem;
use std::ptr::NonNull;

use mach::kern_return::*;
use mach::memory_object_types::memory_object_size_t;
use mach::traps::mach_task_self;
use mach::vm::{mach_make_memory_entry_64, mach_vm_allocate, mach_vm_deallocate, mach_vm_remap};
use mach::vm_inherit::VM_INHERIT_NONE;
use mach::vm_prot::*;
use mach::vm_statistics::{VM_FLAGS_ANYWHERE, VM_FLAGS_FIXED, VM_FLAGS_OVERWRITE};
use mach::vm_types::mach_vm_address_t;

#[inline]
pub fn granularity() -> usize {
	unsafe { mach::vm_page_size::vm_page_size as usize }
}

#[inline(never)]
pub unsafe fn allocate(cap: usize, size_of: usize) -> (NonNull<u8>, usize) {
	if cap == 0 {
		return (NonNull::dangling(), 0);
	}

	// Round capacity to nearest multiple of the system's allocation granularity
	let cap = super::round_capacity(cap, size_of);

	let task = mach_task_self();

	// Reserve memory for twice the capacity
	let mut addr: mach_vm_address_t = 0;
	let ret = mach_vm_allocate(task, &mut addr, (cap + cap) as u64, VM_FLAGS_ANYWHERE);
	if ret != KERN_SUCCESS {
		error("vm_allocate", ret)
	}
	debug_assert!(addr != 0);

	// Allocate the first half of the reserved memory
	let ret = mach_vm_allocate(task, &mut addr, cap as u64, VM_FLAGS_FIXED|VM_FLAGS_OVERWRITE);
	if ret != KERN_SUCCESS {
		error("vm_allocate", ret)
	}

	// Get an object handle to the first memory region
	let mut memory_object_size = cap as memory_object_size_t;
	let mut object_handle = mem::MaybeUninit::uninit();
	let parent_handle = 0;
	let ret = mach_make_memory_entry_64(task, &mut memory_object_size, addr, VM_PROT_READ|VM_PROT_WRITE, object_handle.as_mut_ptr(), parent_handle);
	if ret != KERN_SUCCESS {
		mach_vm_deallocate(task, addr, (cap + cap) as u64);
		error("make_memory_entry_64", ret)
	}

	// Map the first half to the second half using the object handle
	let mut to = (addr as *mut u8).add(cap) as mach_vm_address_t;
	let mut current_prot = mem::MaybeUninit::uninit();
	let mut out_prot = mem::MaybeUninit::uninit();
	let ret = mach_vm_remap(task, &mut to, cap as u64, /*mask:*/0, VM_FLAGS_FIXED|VM_FLAGS_OVERWRITE, task, addr, /*copy:*/0, current_prot.as_mut_ptr(), out_prot.as_mut_ptr(), VM_INHERIT_NONE);
	if ret != KERN_SUCCESS {
		mach_vm_deallocate(task, addr, (cap + cap) as u64);
		error("vm_remap", ret)
	}

	// TODO: object_handle is leaked here. Investigate whether this is ok
	(NonNull::new_unchecked(addr as *mut u8), cap)
}

#[inline]
pub unsafe fn free(ptr: NonNull<u8>, cap: usize) {
	let addr = ptr.as_ptr() as mach_vm_address_t;
	let size = (cap + cap) as u64;
	mach_vm_deallocate(mach_task_self(), addr, size);
}

#[cold]
#[track_caller]
fn error(name: &str, ret: kern_return_t) -> ! {
	panic!("mach_{}(): {}", name, ret)
}
