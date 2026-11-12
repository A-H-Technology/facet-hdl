# facet-hdl

Spike: declare the boundary between an SoC FPGA's ARM side (the HPS) and
its fabric **once**, in Rust, and get both sides from it. The hello world,
`examples/hello`, is one call each way:

```rust
#[facet_hdl::rpc]
pub mod hello {
    /// Runs in the fabric; Rust calls it.
    #[import]
    pub fn add_one(x: u32) -> u32;

    /// Runs here; the fabric calls it each time the button goes down.
    #[export(oneway)]
    pub fn pressed() {
        println!("hello from the fabric!");
    }
}
```

The fabric half is Spade, written against types generated from that module:

```spade
use hello::hello_regs;

pub entity hello_fabric(clk: clock, rst: bool, axi: facet_hdl::AxiIn, button: bool)
    -> (facet_hdl::AxiOut, bool)
{
    let p = inst hello_regs(clk, rst, axi);

    // add_one: answered in the cycle Rust's call arrives.
    set p.add_one_resp.write = match p.add_one_req.read {
        Some(args) => Some(args.x +. 1),
        None => None,
    };
    set p.add_one_req.ack = !p.add_one_resp.full;

    // pressed: one call per press.
    reg(clk) was_down: bool reset(rst: false) initial(false) = button;
    set p.pressed_req.write = if button && !was_down { Some(()) } else { None };

    (p.axi_out, p.irq)
}
```

And the HPS side (`src/bin/hello.rs`) is:

```rust
let fabric: Hello = bind(transport)?;
println!("add_one(41) = {}", block_on(fabric.add_one(41))?);
block_on(fabric.serve())?; // prints "hello from the fabric!" on every press
```

Imports are `async`: a call into the fabric finishes whenever the fabric
answers, so it's a future, and `serve` is one too. They run on any executor
(tokio, smol, or the bundled `block_on` above), and on `&self`, so a host can
serve the fabric's calls and make its own on the same boundary at once.

Calls are built on ports, which you can also declare directly:

```rust
#[derive(Facet)]
pub struct Adder {
    pub operands: ToFabric<Operands>,   // host writes, fabric reads
    pub sum: FromFabric<Sum>,           // fabric drives, host reads
}

#[derive(Facet)]
pub struct Pipe {
    pub jobs: ToFabricQueue<Job, 64>,     // host -> fabric channel
    pub done: FromFabricQueue<Done, 64>,  // fabric -> host channel
}
```

Two kinds of port, each in both directions. `ToFabric`/`FromFabric` are
registers: one value, last writer wins. `ToFabricQueue`/`FromFabricQueue` are
SPSC channels: a ring of `N` entries that never drops or repeats one. The host
gets `push(&T) -> Result<(), PushError>` / `pop() -> Option<T>`, or
`send(&T).await` / `recv().await` to wait for room or for an entry,
and the fabric gets the end of a queue shaped like Spade's own
`std::mem::FifoRead`/`FifoWrite`: `facet_hdl::Read<T> { read: Option<T>,
ack: inv bool }` for a host -> fabric queue, `facet_hdl::Write<T> { write:
inv Option<T>, full: bool }` for a fabric -> host one.

- **Spade** (`facet-hdl-spade`): one Spade type per Rust type (structs,
  enums, arrays, `Option`, `Result`), `pack_*`/`unpack_*` functions between
  each type and its bits, and a `<boundary>_regs(clk, rst, axi)` entity that
  returns one `<Boundary>Ports` struct: the bus's answer, the `irq` line, and
  a field per port (a value for a register, a `Read`/`Write` end for a
  queue). In front of it sits an AXI4 slave (`facet_hdl.spade`) shaped
  for the DE25-Nano's lightweight bridge as its phase 1 sets it up: full
  AXI4, 32-bit data, 4-bit IDs echoed, bursts. Your fabric logic is Spade
  and instantiates the register file; the bus is two struct ports
  (`facet_hdl::AxiIn`/`AxiOut`), and flat `s_axi_*` signals only appear on
  the top level. The Spade compiler runs in-process (`Spade::compile`),
  pinned to v0.21.0, so there's no swim step. Its output is SystemVerilog,
  which chipsmith reads. (Instantiating the register file straight from VHDL
  or Verilog isn't tried: its struct ports would follow Spade's own layout.)
- **Host handles** (`facet-hdl`): `bind::<Adder>(transport)` returns that same
  struct with live handles: `b.leds.write(&ctl)?`, `b.status.read()?`. There's
  no `read` on a `ToFabric` and no `write` on a `FromFabric`.
- **A fingerprint** of the layout sits in word 0 of the register window.
  `bind` refuses a bitstream that was generated from a different declaration.

Target is mercury (Terasic DE25-Nano, Agilex 5) over the lightweight
HPS-to-FPGA bridge, but the pieces are separate crates:

| crate | role |
| --- | --- |
| `facet-hdl-macros` | `#[facet_hdl::rpc]` (re-exported from `facet-hdl`), parsed with unsynn |
| `facet-hdl` | `ToFabric`/`FromFabric` registers, `ToFabricQueue`/`FromFabricQueue` channels, shape → `HwType`, bit codec, `Boundary` layout, `Transport`, `bind` |
| `facet-hdl-spade` | Spade generator, the AXI4 slave, the in-process compiler; `literal(&value)` renders a Rust value as a Spade constant |
| `facet-hdl-iverilog` | Co-simulation: Icarus Verilog runs the compiled fabric, and a generated testbench turns stdin lines into AXI4 transactions, a new ID each. `SimTransport` makes the real host code drive it; `vvp sim.vvp +vcd=<file>` dumps waveforms |
| `facet-hdl-linux` | Linux transports: `DevMem` (uncached `/dev/mem` mapping of the bridge, `AGILEX5_LWH2F = 0x2000_0000`, polls) and `Uio` (a `generic-uio` node: the mapping plus the `irq` line, so `pop_wait` sleeps) |
| `examples/hello` | The hello world above: declaration, the fabric (`hdl/hello_fabric.spade`), the generator bin, the HPS CLI, and its co-sim |

```sh
nix develop --command cargo test                                 # incl. the iverilog co-sims
nix develop --command cargo run --features gen --bin hello-gen   # regenerate examples/hello/hdl/generated
nix build .#hello-hps                                            # static aarch64-musl CLI for mercury
```

## Calls: the fabric calls Rust, Rust calls the fabric

Mark functions in a module. `#[export]` fns are Rust the fabric calls,
`#[import]` fns are bodiless declarations of HDL that Rust calls:

```rust
#[facet_hdl::rpc]
pub mod calc {
    #[export(depth = 8)]            // up to 8 calls in flight
    pub fn add(a: u32, b: u32) -> Option<u32> { a.checked_add(b) }

    #[export]                       // () still answers: done, or panicked
    pub fn apply(cfg: Config) { /* ... */ }

    #[export(oneway)]               // no answer at all; must return ()
    pub fn log(ev: Event) { /* ... */ }

    #[import]                       // implemented in HDL
    pub fn scale(x: u16) -> u32;

    #[import]                       // no arguments, () answer: just handshakes
    pub fn flush();
}

let calc: calc::Calc = bind(transport)?;
let y = calc.scale(7).await?;   // Rust -> fabric
calc.serve().await?;            // fabric -> Rust, forever (or poll())
```

The macro adds an argument struct per fn (`AddArgs { a, b }`) and a boundary
named after the module (`Calc`) with two queues per fn, `<fn>_req` and
`<fn>_resp`:

- **Export** `add`: the fabric pushes an `AddArgs` onto `add_req` and pops a
  `CallResult<T>` from `add_resp`, either `Returned(T)` or `Panicked`. In
  Spade that's `facet_hdl::CallResult<Option<uint<32>>>`, so the fabric
  matches on it like Rust would. The fns stay as written, so Rust can still
  call them.
- **Import** `scale`: the declaration becomes a method,
  `async fn scale(&self, x) -> Result<u32, PortError>`. The fabric pops a
  `ScaleArgs` from `scale_req` and pushes a plain `uint<32>` onto
  `scale_resp`, since HDL can't panic.
- `oneway` drops `_resp`. An exported oneway fn's panic is only reported by
  the panic hook; an imported one's future resolves once the call is queued.
- A fn with no arguments, or an answer of `()`, is a queue of `()`: the
  fabric reads or writes `Some(())`, and no data is stored. The handshake is
  the message.

Calls to one import can overlap (`join_all` over eight `scale`s puts eight
in flight); past `depth` they wait for room. The ring is FIFO, so answer
`k` belongs to request `k`, and an import's `_resp` is an `rpc::Answers`
rather than a `FromFabricQueue`: each call claims its answer by that number,
and whoever pops another call's answer first parks it for them.

There's no timeout parameter: put your runtime's timeout around the call
(or use `block_on_timeout`). Dropping the future abandons the call. The
request may already be with the fabric, and its answer still comes, but it
gets dropped on arrival instead of being handed to a later call. Answers
owed to a previous process are dropped the same way.

One thread per binding waits on the irq line and wakes whichever futures
are waiting, so nothing needs a reactor. It only touches the line while
someone is waiting. If the fabric stops draining an export's `_resp`,
`serve` blocks on it (there's no "room" interrupt) and stops answering other
fns until it drains.

Limits for now: no generics, no `self`, parameters are `name: Type`, and
exports are sync fns.

## Wire format

These are the rules `codec.rs` and the generated Spade both implement. The
Spade side packs and unpacks explicitly, so the format doesn't depend on how
Spade lays its own types out.

- `bool` = 1 bit. `u8..u128`/`i8..i128` = their width, two's complement.
  A fieldless enum is its declaration index in `ceil(log2(n))` bits. Structs
  are their fields in declaration order with the first field at the LSB.
  Arrays are their elements with index 0 at the LSB.
- An enum whose variants carry data (`Option<T>`, `Result<T, E>`, or your own
  `Cmd::Set(Config)`) is a tag, the variant's declaration index in
  `ceil(log2(n))` bits, then a payload area as wide as the widest payload.
  The payload starts at the area's LSB and the bits above it are zero; a
  decoder ignores them. A `()` payload takes no bits, so `Result<(), E>` is
  just the tag and `E`. A variant carries at most one field: wrap several in
  a struct. `None` is variant 0 and `Some` 1; `Ok` is 0 and `Err` 1.
  In Spade it's the same enum: `Option`/`Result` are Spade's own, and a user
  enum's payload variant is declared `Set { value: Config }`.
- Rejected with an error that names the field path: `usize`, floats, `char`,
  strings, slices, variants with several fields, zero-sized things,
  recursive types.
  Names that are Spade keywords (`reg`, `port`, `set`, ...), or ports whose
  generated internals would collide, are rejected too.
- `()` can only be a whole queue entry, which has no entry words and no
  storage in the fabric. The host pops by writing the tail as usual,
  and pushes by writing the new absolute head to the head word. There's no
  entry word to commit on, and writing the absolute count, like a pop does,
  makes a retried write harmless.
- Each register port occupies `ceil(width/32)` consecutive 32-bit words, in
  field order, starting at word 1. A queue port occupies two counter words
  (`+0` tail, `+1` head) followed by one entry's worth of words.
- A **ToFabric** write commits when its last word is written. Earlier words
  are buffered, so the fabric never sees a torn value, and `<port>_written`
  pulses once per commit.
- A **FromFabric** read snapshots the whole value when its first word is read,
  and the later words come from that snapshot.
- A `FromFabric` read returns what the fabric drives *now*. It is not a
  response to an earlier `ToFabric` write, however close together the two
  are. Request/response over plain ports needs a field the fabric bumps once
  the answer is valid (a generation counter, like `Sum::calls`), and the host
  waits for it with `FromFabric::read_until`; `add` in
  `crates/facet-hdl-spade/tests/registers.rs` shows the pattern.
  A `ToFabric` port is a mailbox, so a write replaces a request still in
  flight. Queue ports are for traffic that can't lose messages.
- A **queue** has a free-running u32 head (producer count) and tail (consumer
  count). Each counter has exactly one writer, so the bus needs no locks or
  atomics. `used = head - tail`, which keeps full and empty distinct. The depth
  is a power of two from 2 to 65536, and it's part of the fingerprint.
  - Host → fabric: the host writes the entry window, and its last word
    enqueues. The host caches the fabric's tail and re-reads it only when the
    ring looks full, so a push normally costs no reads at all.
  - Fabric → host: the entry window shows the oldest entry, snapshotted on its
    first word. To pop, the host writes the new **absolute** tail, never a
    pop-on-read: a debug dump or a retried read must not eat entries, and the
    fabric ignores a tail write that isn't a step forward.
  - Full and empty only show up in the counters, never as an AXI error. An
    error response can arrive as an SError and panic the kernel. If the
    fabric's counters ever disagree with the host's (`head - tail > N`, or a
    counter the host owns moving on its own), that's `PortError::Desync`; it
    is never used as an index. `bind` starts from the fabric's counters, so a
    restarted host resumes where the ring really is.
- Words from two accessors interleaving on one port would commit or return
  a value neither of them meant. So a binding owns its whole register window:
  `bind` takes an exclusive `flock` on the transport's lock file and holds it
  until the binding is dropped. DevMem and Uio both use
  `/run/lock/facet-hdl-window-<phys>.lock`, so they exclude each other too. A second `bind`, in the same
  process or another one, gets `BindError::Busy`. The kernel releases the lock
  when the holder dies, `kill -9` included. Within one binding, a mutex keeps
  each port access contiguous. Raw `devmem` pokes bypass all of this; only a
  kernel driver owning the window could stop them.
- The register file's `irq` output is **level**-sensitive: high while any
  `FromFabricQueue` holds an entry. A consumer that finds its queue empty and
  then waits can't miss an entry that landed in between, because the line is
  already high when the wait starts. `FromFabricQueue::recv()` (and its
  sync twin `pop_wait(timeout)`) sleeps on it when the transport offers an
  `Interrupt` (`Uio`, and the iverilog sim built with `.irq()`), and polls
  every 50 µs otherwise (`DevMem`).
  - There is one line for the whole boundary. A waiter woken by another
    queue's entry re-checks its own queue and sleeps again. While that other
    entry sits unconsumed, though, the line stays high and the waiter
    degrades to polling. Keep every `FromFabricQueue` drained.
  - Nothing signals "a `ToFabricQueue` has room again". A level "not full"
    line would be high nearly all the time, so a full push returns
    `PushError::Full`, and `send` re-polls every 50 µs until there's room.
- Every AXI access answers OKAY: unmapped words read 0 and ignore writes,
  since an error response can take the kernel down. Each beat is one 32-bit
  word; AxSIZE and WSTRB are ignored, and every burst counts up.
- Generated registers start at their reset values (`initial`), as FPGA flops
  power up, so fabric logic that isn't reset with the register file never
  sees undefined state.

## Verified

Every co-sim runs the real host API against Spade compiled in-process and
simulated by Icarus Verilog. Each was also checked by planting a bug and
watching it fail.

- The unit tests cover the codec, layout, macro expansion and its errors,
  and bind. There's an in-memory fake fabric for fingerprint mismatch and
  bad discriminants.
- AXI4 slave on its own: single beats and 9- and 16-beat bursts, every
  response's ID checked by the testbench. A slave that doesn't echo BID, or
  whose bursts don't advance, fails.
- Echo co-sim covers nested structs, arrays of structs/enums/bools, 2-D
  arrays, `u128`, `i8`/`i64`, and tagged types: `Option<u16>`,
  `Result<(), Tri>`, an array of a user enum with payloads of three widths,
  and `Option<Result<Inner, bool>>`. A golden value written as a Spade
  literal checks packing and unpacking *separately*, so a symmetric bug can't
  hide behind the loopback. A planted field-order bug fails the packing
  check, and a payload read one bit low fails the unpacking one.
- Hello co-sim: Rust's `add_one(41)` comes back 42 (and `u32::MAX`
  wraps to 0), and one 20-cycle button press calls `pressed` exactly once.
  A fabric answering `x` instead of `x + 1` fails, and so does one without
  the edge detection (more than one call per press). The checked-in
  `hdl/generated/` has to match the declaration.
- Register co-sim (an adder over plain registers): bind/fingerprint, 3-word
  signed sums, a snapshotted 64-bit cycle counter, a 200-cycle adder showing
  write-then-read returning a stale value and the generation-counter wait
  returning the right one, and a torn commit through an unowned second
  accessor.
- Queue co-sim: a Spade pipeline (`jobs` → ×3 → `done`) runs 300 sequenced
  jobs both ways at full rate, once free-running and once with an LFSR
  stalling it. It checks for gaps, duplicates and corrupt 2- and 3-word
  entries, and backpressure has to actually happen. Another test pushes into
  a frozen consumer: the ninth push returns `Full`, and the fabric's counters
  are unchanged. A fabric→host ring planted to accept pushes when full fails
  both rate tests as a `Desync`.
- Interrupt: the line is low while the results queue is empty, high while a
  result waits, stays high until it's popped, then goes low. `pop_wait`
  wakes far inside its timeout and returns `None` once the timeout passes.
  With `irq` planted stuck low, the line test fails and `pop_wait` only
  wakes at its 30 s timeout.
- Window ownership: a second `bind` is refused in-process and from another
  process, and a holder killed with `kill -9` frees the window.
- RPC co-sim: the fabric, in Spade, is the caller. It fires 64 `add` calls
  back to back, 8 in flight, alongside 16 `check` calls, one of which panics
  in Rust, checks every answer by matching on `CallResult`/`Option`/`Result`,
  and reports its tally through a oneway call. Planting an off-by-one in
  Rust's `add` makes it report 56 mismatches, the 56 calls that don't
  overflow. Its counters have no reset on purpose: with the register file's
  queues ready during reset, calls get lost and the test fails.
- Import co-sim, on tokio: Rust calls the Spade fabric's `scale` (also 8
  calls in flight at once), `ping` (no arguments, `()` answer), oneway
  `set_led` read back through `led`, while `serve` answers the fabric's three
  calls to the no-argument export `tick` alongside. `slow` answers 2000
  cycles late, one at a time:
  - `slow(30)` and `slow(40)` both go out, then `slow(40)` is awaited first
    and has to park `slow(30)`'s answer on the way. If other calls' answers
    are dropped, `slow(30)` never returns. If answers are handed out in pop
    order, `slow(40)` gets 31.
  - `slow(1)` is given up on after its request went out, and `slow(2)` must
    still return 3. The same holds across a rebind, for an answer owed to
    the dropped binding.
  - With pop-order hand-out planted, that returns `slow(1)`'s 2.
- `Uio`'s register path against a faked `/sys/class/uio` and a plain file in
  place of `/dev/uioN`: lookup by name, the mapping, bounds, a node whose
  `reg` is smaller than the boundary, and the shared lock path. **Not
  verified:** `UioIrq` (unmask by writing 1, `poll`, read the count). That
  needs a real `uio_pdrv_genirq` device.

## Not yet: running on mercury

Nothing has touched hardware. `hello-hps` builds. chipsmith builds and loads
DE25-Nano cores itself now, and mercury boots chipsmith's HPS-first phase 1
from QSPI (chipsmith 98c0898), so what's left is the bridge:

1. **The bridge's fabric-side ports.** Phase 1 exports the lightweight
   bridge as full AXI4 (32-bit data, 29-bit address, 4-bit IDs, bursts),
   which is what `facet_hdl.spade`'s slave speaks. chipsmith/a5pnr don't map
   those ~200 HPS signals as nodes yet; only `f2h_irq0` is.
2. **Enabling the bridge after a load from Linux.** U-Boot's `bridge enable`
   runs before the fabric exists, and the kernel has no Agilex bridge
   driver; the ATF SMC `HPS_SET_BRIDGES` is the way in, likely a small
   module in nixos-fpga.
3. **For `Uio` and its interrupt:** a device-tree node with
   `compatible = "generic-uio"`, `reg` covering the window, and `irq` wired
   to an FPGA-to-HPS interrupt as a level-high SPI (`f2h_irq0` already works
   on the board), plus `uio_pdrv_genirq.of_id=generic-uio` on the kernel
   command line. This belongs in nixos-fpga.
4. **Then:** `hello --uio <node name>`, or `hello --base
   0x2000_0000+<component offset>` without the node, with `button` tied to
   one of the board's keys.

## Licence

EUPL-1.2, see [LICENSE](LICENSE), from c1db5fe on. Earlier commits were
published under MIT OR Apache-2.0 and stay available under those terms.
