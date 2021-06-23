/*!

*/

use std::{borrow, cmp, hint, iter, mem, ops, ptr, slice};
use std::ptr::NonNull;

mod platform;

/// Ring buffer backed by mirrored virtual memory.
#[derive(Debug)]
pub struct RingBuffer<T> {
	ptr: NonNull<T>,
	// Capacity of the ring buffer in bytes.
	// Twice as much virtual memory has been allocated.
	cap: usize,
	// Base offset to the start of the ring buffer (in bytes).
	// Note this must always point within the lower half of the virtual memory!
	// Ie. assert!(self.base < self.cap);
	base: usize,
	// Number of valid elements in the ring buffer (in # elements).
	len: usize,
}

impl<T> RingBuffer<T> {
	/// Constructs a new, empty `RingBuffer<T>`.
	///
	/// The ring buffer will not allocate until elements are pushed onto it.
	#[inline]
	pub const fn new() -> RingBuffer<T> {
		RingBuffer { ptr: NonNull::dangling(), cap: 0, base: 0, len: 0 }
	}

	/// Constructs a new, empty `RingBuffer<T>` with the specified capacity.
	///
	/// The ring buffer's capacity will be rounded up to the nearest multiple of the system's allocation granularity.
	/// If `capacity` is 0, the ring buffer will not allocate.
	///
	/// It is important to note that although the returned ring buffer has the _capacity_ specified,
	/// the ring buffer will have a zero _length_.
	///
	/// # Panics
	///
	/// Panics if the capacity exceeds system limits or there is not enough contigious memory for 2x the requested capacity.
	#[inline]
	pub fn with_capacity(capacity: usize) -> RingBuffer<T> {
		if capacity == 0 {
			return RingBuffer::new();
		}

		let (ptr, cap) = unsafe { platform::allocate(capacity, mem::size_of::<T>()) };
		let ptr = ptr.cast();

		RingBuffer { ptr, cap, base: 0, len: 0 }
	}

	/// Returns the number of elements the ring buffer can hold without reallocating.
	#[inline]
	pub fn capacity(&self) -> usize {
		self.cap / mem::size_of::<T>()
	}

	/// Returns the number of elements in the ring buffer, also referred to as its ‘length’.
	#[inline]
	pub fn len(&self) -> usize {
		self.len
	}

	/// Returns `true` if the ring buffer contains no elements.
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// Extracts a slice containing the entire ring buffer.
	#[inline]
	pub fn as_slice(&self) -> &[T] {
		unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
	}

	/// Extracts a mutable slice of the entire ring buffer.
	#[inline]
	pub fn as_mut_slice(&mut self) -> &mut [T] {
		unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
	}

	/// Returns a raw pointer to the ring buffer's first element.
	///
	/// The caller must ensure that the ring buffer outlives the pointer this function returns, or else it will end up pointing to garbage.
	/// Modifying the ring buffer may cause its buffer to be reallocated, which would also make any pointers to it invalid.
	///
	/// The caller must also ensure that the memory the pointer (non-transitively) points to is never written to (except inside an `UnsafeCell`)
	/// using this pointer or any pointer derived from it. If you need to mutate the contents of the slice, use [`as_mut_ptr`](Self::as_mut_ptr).
	#[inline]
	pub fn as_ptr(&self) -> *const T {
		unsafe {
			(self.ptr.as_ptr() as *const u8).add(self.base) as *const T
		}
	}

	/// Returns an unsafe mutable pointer to the ring buffer's first element.
	///
	/// The caller must ensure that the ring buffer outlives the pointer this function returns, or else it will end up pointing to garbage.
	/// Modifying the ring buffer may cause its buffer to be reallocated, which would also make any pointers to it invalid.
	#[inline]
	pub fn as_mut_ptr(&mut self) -> *mut T {
		unsafe {
			(self.ptr.as_ptr() as *mut u8).add(self.base) as *mut T
		}
	}

	/// The number of additional elements available in the ring buffer.
	#[inline]
	pub fn reserved_len(&self) -> usize {
		self.cap / mem::size_of::<T>() - self.len
	}

	/// Returns a pointer the remaining spare capacity of the ring buffer.
	#[inline]
	pub fn reserved_ptr(&mut self) -> *mut T {
		unsafe { self.as_mut_ptr().add(self.len) }
	}

	/// Returns the remaining spare capacity of the ring buffer as a slice of `MaybeUninit<T>`.
	///
	/// The returned slice can be used to fill the vector with data (e.g. by reading from a socket)
	/// before marking the data as initialized using the [`add_len`](Self::add_len) method.
	pub fn reserved_mut(&mut self) -> &mut [mem::MaybeUninit<T>] {
		unsafe {
			let ptr = self.reserved_ptr();
			let len = self.reserved_len();
			mem::transmute(ptr::slice_from_raw_parts_mut(ptr, len))
		}
	}

	/// Forces the length of the ring buffer to include `additional` number of elements.
	///
	/// This is a low-level operation that maintains none of the normal invariants of the type.
	/// Normally changing the lenght of a ring buffer is done using one of the safe operations instead.
	///
	/// # Safety
	///
	/// * `len + additional` must be less than or equal to [`capacity`](Self::capacity).
	/// * The elements at `len..new_len` must be initialized.
	#[inline]
	pub unsafe fn add_len(&mut self, additional: usize) {
		self.len += additional;
	}

	/// Forces the length of the ring buffer to `new_len`.
	///
	/// This is a low-level operation that maintains none of the normal invariants of the type.
	/// Normally changing the length of a ring buffer is done using one of the safe operations instead.
	///
	/// # Safety
	///
	/// * `new_len` must be less than or equal to [`capacity`](Self::capacity).
	/// * The elements at `len..new_len` must be initialized.
	#[inline]
	pub unsafe fn set_len(&mut self, new_len: usize) {
		self.len = new_len;
	}

	/// Clears the ring buffer, removing all values.
	///
	/// Note that this method has no effect on the allocated capacity of the ring buffer.
	pub fn clear(&mut self) {
		let len = self.len;
		self.len = 0;
		unsafe {
			ptr::slice_from_raw_parts_mut(self.as_mut_ptr(), len).drop_in_place();
		}
	}

	/// Shortens the ring buffer, keeping the first `len` elements and dropping the rest.
	///
	/// If `len` is greater than the ring buffer’s current length, this has no effect.
	///
	/// Note that this method has no effect on the allocated capacity of the vector.
	#[inline]
	pub fn truncate(&mut self, len: usize) {
		if len > self.len {
			return;
		}
		unsafe {
			let remaining_len = self.len - len;
			let s = ptr::slice_from_raw_parts_mut(self.as_mut_ptr().add(len), remaining_len);
			self.len = len;
			s.drop_in_place();
		}
	}

	/// Removes `n` elements from the tail.
	pub fn remove_tail(&mut self, n: usize) {
		// Keep the method safe by removing max of `len` elements
		let n = cmp::min(self.len, n);
		// Pointer to the removed elements
		let s = ptr::slice_from_raw_parts_mut(self.as_mut_ptr(), n);
		// Remove the elements first
		self.len -= n;
		self.base += n * mem::size_of::<T>();
		// Drop the elements
		unsafe { s.drop_in_place(); }
		// Adjust the base offset
		if self.base >= self.cap {
			self.base -= self.cap;
		}
	}

	/// Appends an element to the front.
	///
	/// # Panics
	///
	/// Panics if the new capacity fails to allocate.
	#[inline]
	pub fn push(&mut self, value: T) {
		self.reserve(1);
		unsafe {
			self.as_mut_ptr().add(self.len).write(value);
			self.len += 1;
		}
	}

	/// Returns the back element from a ring buffer and returns it, or [`None`] if it is empty.
	#[inline]
	pub fn pop(&mut self) -> Option<T> {
		if self.len == 0 {
			return None;
		}
		unsafe {
			let s = self.as_mut_ptr();
			self.len -= 1;
			self.base += mem::size_of::<T>();
			let value = s.read();
			// Adjust the base offset
			if self.base >= self.cap {
				self.base -= self.cap;
			}
			Some(value)
		}
	}

	/// Copies and appends all elements in a slice to the `RingBuffer`.
	///
	/// Note that this function is same as `extend` except that it is specialized to work with slices instead.
	/// If and when Rust gets specialization this function will likely be deprecated (but still available).
	#[inline]
	pub fn extend_from_slice(&mut self, other: &[T]) where T: Copy {
		self.reserve(other.len());
		unsafe {
			other.as_ptr().copy_to_nonoverlapping(self.as_mut_ptr().add(self.len), other.len());
			self.len += other.len();
		}
	}

	/// Resizes the `RingBuffer` in-place so that `len` is equal to `new_len`.
	#[inline]
	pub fn resize(&mut self, new_len: usize, value: T) where T: Clone {
		self.resize_with(new_len, move || value.clone())
	}

	/// Resizes the `RingBuffer` in-place so that `len` is equal to `new_len`.
	#[inline]
	pub fn resize_with<F: FnMut() -> T>(&mut self, new_len: usize, mut f: F) {
		if new_len <= self.len() {
			self.truncate(new_len);
		}
		else {
			let additional = new_len - self.len();
			self.reserve(additional);
			unsafe {
				let mut ptr = self.reserved_ptr();
				for _ in 0..additional {
					ptr.write(f());
					ptr = ptr.add(1);
				}
				self.set_len(new_len);
			}
		}
	}

	/// Reserves capacity for at least `additional` more elements to be inserted in the given `RingBuffer<T>`.
	///
	/// The collection may reserve more space to avoid frequent reallocations.
	/// After calling `reserve`, capacity will be greater than or equal to `self.len() + additional`.
	/// Does nothing if capacity is already sufficient.
	///
	/// # Safety
	///
	/// After this method the ring buffer is guaranteed to contain room for at least `additional` elements.
	#[inline]
	pub fn reserve(&mut self, additional: usize) {
		unsafe {
			if additional > self.reserved_len() {
				self.reallocate(additional);
			}
			// Teach the compiler that there are at least additional extra elements available after this point
			if additional > self.reserved_len() {
				hint::unreachable_unchecked()
			}
		}
	}

	#[inline(never)]
	unsafe fn reallocate(&mut self, additional: usize) {
		let capacity = match self.len.checked_add(additional) {
			Some(capacity) => capacity,
			None => platform::invalid_capacity(additional),
		};

		// Allocate new RingBuffer
		let (ptr, cap) = platform::allocate(capacity, mem::size_of::<T>());
		let ptr = ptr.cast();

		// Construct new RingBuffer
		let mut rb = RingBuffer { ptr, cap, base: 0, len: 0 };

		// Copy over the elements from the old ring buffer
		self.as_ptr().copy_to_nonoverlapping(rb.as_mut_ptr(), self.len);

		// Copy over the length and empty the current ring buffer
		// No destructors are ran since the elements are moved over
		rb.len = self.len;
		self.len = 0;

		// Drop self and replace with reallocated ring buffer
		*self = rb;
	}
}

impl<T> Drop for RingBuffer<T> {
	fn drop(&mut self) {
		unsafe {
			let len = self.len;
			self.len = 0;
			ptr::slice_from_raw_parts_mut(self.as_mut_ptr(), len).drop_in_place();
			platform::free(self.ptr.cast(), self.cap);
		}
	}
}

impl<T> ops::Deref for RingBuffer<T> {
	type Target = [T];
	#[inline]
	fn deref(&self) -> &[T] {
		self.as_slice()
	}
}
impl<T> ops::DerefMut for RingBuffer<T> {
	#[inline]
	fn deref_mut(&mut self) -> &mut [T] {
		self.as_mut_slice()
	}
}
impl<T> AsRef<[T]> for RingBuffer<T> {
	#[inline]
	fn as_ref(&self) -> &[T] {
		self.as_slice()
	}
}
impl<T> AsMut<[T]> for RingBuffer<T> {
	#[inline]
	fn as_mut(&mut self) -> &mut [T] {
		self.as_mut_slice()
	}
}
impl<T> borrow::Borrow<[T]> for RingBuffer<T> {
	#[inline]
	fn borrow(&self) -> &[T] {
		self.as_slice()
	}
}
impl<T> borrow::BorrowMut<[T]> for RingBuffer<T> {
	#[inline]
	fn borrow_mut(&mut self) -> &mut [T] {
		self.as_mut_slice()
	}
}

impl<T> Extend<T> for RingBuffer<T> {
	#[inline]
	fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
		// Not very efficient...
		let iter = iter.into_iter();
		self.reserve(iter.size_hint().0);
		for value in iter {
			self.push(value);
		}
	}
}
impl<T> iter::FromIterator<T> for RingBuffer<T> {
	#[inline]
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> RingBuffer<T> {
		// Not very efficient...
		let iter = iter.into_iter();
		let mut rb = RingBuffer::with_capacity(iter.size_hint().0);
		for value in iter {
			rb.push(value);
		}
		rb
	}
}
impl<T: Clone> Clone for RingBuffer<T> {
	#[inline]
	fn clone(&self) -> RingBuffer<T> {
		self.as_slice().iter().cloned().collect()
	}
}

// Safe because it is possible to free this from a different thread
unsafe impl<T: Send> Send for RingBuffer<T> {}
// Safe because this doesn't use any kind of interior mutability
unsafe impl<T: Sync> Sync for RingBuffer<T> {}
