use valgrind_requests::{self, valgrind, valgrind_println_unchecked};
use valgrind_requests_tests::MARKER;

fn main() {
    let _ = MARKER;
    // SAFETY: This standalone test intentionally exercises the unchecked macro with a static
    // format string and no format arguments.
    #[cfg_attr(not(feature = "_act"), expect(unused_unsafe))]
    unsafe {
        valgrind_println_unchecked!("{MARKER}");
    }
    let native = valgrind::running_on_valgrind() == 0;

    let result = valgrind::non_simd_call0(|tid| -> usize { tid + 2 });
    assert_eq!(result, if native { 0 } else { 3 });

    {
        let vec: Vec<u8> = vec![0, 1, 2, 3, 4, 5];
        let pool = vec.as_ptr().cast::<()>();

        valgrind::create_mempool(pool, 0, true);
        if valgrind::mempool_exists(pool) {
            valgrind::destroy_mempool(pool);
        }

        drop(vec);

        // This'll provoke an error because of an illegal memory access which is reported by
        // valgrind and tells us that our request is working
        valgrind::destroy_mempool(pool);
    }

    std::process::exit(i32::from(valgrind::running_on_valgrind() != 0));
}
