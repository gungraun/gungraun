use client_request_tests::MARKER;
use valgrind_requests::{self, valgrind, valgrind_println_unchecked};

fn main() {
    unsafe { valgrind_println_unchecked!("{MARKER}") };
    let native = valgrind::running_on_valgrind() == 0;

    let result = valgrind::non_simd_call0(|tid| -> usize { tid + 2 });
    assert_eq!(result, if native { 0 } else { 3 });

    {
        let vec: Vec<u8> = vec![0, 1, 2, 3, 4, 5];
        let pool = vec.as_ptr() as *const ();

        valgrind::create_mempool(pool, 0, true);
        if valgrind::mempool_exists(pool) {
            valgrind::destroy_mempool(pool);
        }

        drop(vec);

        // This'll provoke an error because of an illegal memory access which is reported by
        // valgrind and tells us that our request is working
        valgrind::destroy_mempool(pool);
    }

    std::process::exit(valgrind::running_on_valgrind() as i32);
}
