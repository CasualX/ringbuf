use vringbuf::RingBuffer;

#[test]
fn test_looptheloop() {
	// Empty ring buffer
	let mut rbuf = RingBuffer::<u8>::new();

	// Reallocate the buffer
	rbuf.reserve(1);

	// Go through the ring buffer a couple of times
	let cap = dbg!(rbuf.capacity());
	for _ in 0..100 {
		let len = cap * 2 / 3;
		for _ in 0..len {
			rbuf.push(0xfe);
		}
		assert_eq!(rbuf.len(), len);
		for &el in &rbuf[..] {
			assert_eq!(el, 0xfe);
		}
		rbuf.clear();
	}
}
