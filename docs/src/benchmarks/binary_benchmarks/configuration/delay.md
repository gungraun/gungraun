# Delay the Command

Delaying the execution of the `Command` with `Command::delay` might be necessary
if the `setup` is executed in parallel either with `Command::setup_parallel` or
`Command::stdin` set to `Stdin::Setup`.

For example, if you have a server which needs to be started in the setup to be
able to benchmark a client (in our example a crate's binary simply named
`client`):

```rust
# extern crate gungraun;
# macro_rules! env { ($m:tt) => {{ "/some/path" }} }
use std::net::{SocketAddr, TcpListener};
use std::time::Duration;
use std::thread;

use gungraun::prelude::*;
use gungraun::{Delay, DelayKind};

const ADDRESS: &str = "127.0.0.1:31000";

fn setup_tcp_server() {
    println!("Waiting to start server...");
    thread::sleep(Duration::from_millis(300));

    println!("Starting server...");
    let listener = TcpListener::bind(
            ADDRESS.parse::<SocketAddr>().unwrap()
        ).unwrap();

    thread::sleep(Duration::from_secs(1));

    drop(listener);
    println!("Stopped server...");
}

#[binary_benchmark(setup = setup_tcp_server())]
fn bench_client() -> gungraun::Command {
    gungraun::Command::new(env!("CARGO_BIN_EXE_client"))
        .setup_parallel(true)
        .delay(
            Delay::new(DelayKind::TcpConnect(
                ADDRESS.parse::<SocketAddr>().unwrap(),
            ))
            .timeout(Duration::from_millis(500)),
        )
        .build()
}

binary_benchmark_group!(name = my_group, benchmarks = bench_client);
# fn main() {
main!(binary_benchmark_groups = my_group);
# }
```

The server is started in the parallel setup function `setup_tcp_server` since
`Command::setup_parallel` is set to true. The delay of the `Command` is
configured with `Delay` in `Command::delay` to wait for the tcp connection to be
available. We also applied a timeout of `500` milliseconds with
`Delay::timeout`, so if something goes wrong in the server and the tcp
connection cannot be established, the benchmark exits with an error after `500`
milliseconds instead of hanging forever. After the successful delay, the actual
client is executed and benchmarked. After the exit of the client, the setup is
waited for to exit successfully. Then, if present, the `teardown` function is
executed.

Please see the library documentation for all possible [`DelayKind`]s and more
details on the [`Delay`].

[`DelayKind`]: https://docs.rs/gungraun/0.20.0/gungraun/enum.DelayKind.html
[`Delay`]: https://docs.rs/gungraun/0.20.0/gungraun/struct.Delay.html
