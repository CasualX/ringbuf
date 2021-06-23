
use vringbuf::RingBuffer;

static DATA: [u8; 400] = [0xef; 400];

#[test]
fn main() {
	let mut rbuf = RingBuffer::<u8>::with_capacity(1);

	for _ in 0..1000 {
		rbuf.extend_from_slice(&DATA);
		rbuf.remove_tail(DATA.len());
	}
}
