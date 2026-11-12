//! `<boundary>_regs`: the AXI4 slave from `facet_hdl.spade` in front of the
//! boundary's ports, as typed Spade ports.
//!
//! Protocol, matching `facet_hdl::port`:
//! - Word 0 reads the fingerprint.
//! - A `ToFabric` port buffers every word but its last; writing the last word
//!   commits all of them at once and raises `<port>_written` for one cycle.
//! - Reading word 0 of a `FromFabric` port snapshots the whole value; its
//!   later words come from that snapshot.
//! - Queues: see `facet_hdl::queue`. Each counter has one writer.
//! - Every access answers OKAY. Unmapped words read 0 and ignore writes: on
//!   arm64 an error response can arrive as an SError and take the kernel down.

use crate::names::{NameError, check, snake, spade_type};
use crate::types::{pack, unpack};
use facet_hdl::{Boundary, Direction, FINGERPRINT_WORD, PortDecl, PortKind, queue};
use std::collections::HashSet;
use std::fmt::Write;

pub fn entity_name(b: &Boundary) -> String {
    format!("{}_regs", snake(b.name))
}

#[derive(Default)]
struct Parts {
    ports: String,
    body: String,
    /// `word => expression` arms of the read mux.
    reads: String,
    irq: Vec<String>,
}

/// Generated identifiers share one scope with the ports, so two ports
/// can't produce the same internal name.
struct Scope(HashSet<String>);

impl Scope {
    fn claim(&mut self, ident: String, origin: &str) -> Result<String, NameError> {
        check(&ident, origin)?;
        if !self.0.insert(ident.clone()) {
            return Err(NameError::Clash {
                ident,
                origin: origin.to_owned(),
            });
        }
        Ok(ident)
    }
}

/// `uint<from>` expression `bits`, as the 32-bit word starting at bit `lo`.
fn word_of(bits: &str, from: u32, lo: u32) -> String {
    match (from, lo) {
        (32, 0) => bits.to_owned(),
        (_, 0) => format!("trunc({bits})"),
        _ => format!("trunc({bits} >> {lo})"),
    }
}

/// `uint<from>` expression `bits` widened to `uint<to>`.
fn widen(bits: &str, from: u32, to: u32) -> String {
    if from == to {
        bits.to_owned()
    } else {
        format!("zext({bits})")
    }
}

/// The `words` (low first, 32 bits each) as one `uint<width>`.
fn assemble(words: &[String], width: u32) -> String {
    let mut it = words.iter().rev();
    let mut full = it.next().expect("a word").clone();
    for w in it {
        full = format!("{full}.concat({w})");
    }
    let span = words.len() as u32 * 32;
    if span == width {
        full
    } else {
        format!("{{ let full: uint<{span}> = {full}; trunc(full) }}")
    }
}

pub fn generate(b: &Boundary) -> Result<String, NameError> {
    let entity = entity_name(b);
    let mut scope = Scope(HashSet::new());
    for p in [
        "clk", "rst", "axi", "axi_out", "irq", "bus", "wr", "wr_word", "wr_data", "rd", "rd_word", "rdata",
    ] {
        scope.claim(p.into(), "the register file itself")?;
    }
    let mut all = Parts::default();
    writeln!(all.reads, "        {FINGERPRINT_WORD} => {:#x},", b.fingerprint()).unwrap();
    for p in &b.ports {
        let part = match p.kind {
            PortKind::Register => register(p, &mut scope)?,
            PortKind::Queue { depth } => queue_port(p, depth, &mut scope)?,
        };
        all.ports.push_str(&part.ports);
        all.body.push_str(&part.body);
        all.reads.push_str(&part.reads);
        all.irq.extend(part.irq);
    }
    let irq = if all.irq.is_empty() {
        "false".to_owned()
    } else {
        all.irq.join(" || ")
    };
    let Parts { ports, body, reads, .. } = all;
    Ok(format!(
        r#"pub entity {entity}(
    clk: clock,
    rst: bool,
    axi: facet_hdl::AxiIn,
    axi_out: inv facet_hdl::AxiOut,
    // Level-sensitive: high while any fabric -> host queue holds an entry.
    irq: inv bool,
{ports}) {{
    decl rdata;
    let bus = inst facet_hdl::axi4_slave(clk, rst, axi, rdata);
    set axi_out = bus.out;
    let (wr, wr_word, wr_data) = match bus.write {{
        Some(w) => (true, w.word, w.data),
        None => (false, 0, 0),
    }};
    let (rd, rd_word) = match bus.read {{
        Some(word) => (true, word),
        None => (false, 0),
    }};
{body}
    let rdata: uint<32> = match rd_word {{
{reads}        _ => 0,
    }};
    set irq = {irq};
}}
"#
    ))
}

fn register(p: &PortDecl, scope: &mut Scope) -> Result<Parts, NameError> {
    let mut out = Parts::default();
    let n = scope.claim(p.name.into(), "a port")?;
    let origin = format!("internals of port `{n}`");
    let (ty, w, k) = (spade_type(&p.ty), p.ty.width(), p.words);
    let span = k * 32;
    let first = p.word;
    let last = p.word + k - 1;
    match p.direction {
        Direction::ToFabric => {
            let written = scope.claim(format!("{n}_written"), "the commit strobe of a port")?;
            let reg = scope.claim(format!("{n}_reg"), &origin)?;
            let strobe = scope.claim(format!("{n}_strobe"), &origin)?;
            let commit = scope.claim(format!("{n}_commit"), &origin)?;
            writeln!(
                out.ports,
                "    // Host -> fabric, words {first}..={last}\n    {n}: inv {ty},\n    {written}: inv bool,"
            )
            .unwrap();
            let mut words = Vec::new();
            for i in 0..k - 1 {
                let s = scope.claim(format!("{n}_s{i}"), &origin)?;
                writeln!(
                    out.body,
                    "    reg(clk) {s}: uint<32> reset(rst: 0) initial(0) = if wr && wr_word == {} {{ wr_data }} else {{ {s} }};",
                    first + i
                )
                .unwrap();
                words.push(s);
            }
            words.push("wr_data".into());
            writeln!(
                out.body,
                "    let {commit} = wr && wr_word == {last};\n    reg(clk) {reg}: uint<{w}> reset(rst: 0) initial(0) = if {commit} {{ {} }} else {{ {reg} }};\n    reg(clk) {strobe}: bool reset(rst: false) initial(false) = {commit};\n    set {n} = {};\n    set {written} = {strobe};",
                assemble(&words, w),
                unpack(&reg, &p.ty)
            )
            .unwrap();
            for i in 0..k {
                writeln!(
                    out.reads,
                    "        {} => {{ let full: uint<{span}> = {}; {} }},",
                    first + i,
                    widen(&reg, w, span),
                    word_of("full", span, i * 32)
                )
                .unwrap();
            }
        }
        Direction::FromFabric => {
            let packed = scope.claim(format!("{n}_packed"), &origin)?;
            writeln!(
                out.ports,
                "    // Fabric -> host, words {first}..={last}\n    {n}: {ty},"
            )
            .unwrap();
            writeln!(
                out.body,
                "    let {packed}: uint<{span}> = {};",
                widen(&pack(&n, &p.ty), w, span)
            )
            .unwrap();
            writeln!(out.reads, "        {first} => {},", word_of(&packed, span, 0)).unwrap();
            if k > 1 {
                let snap = scope.claim(format!("{n}_snap"), &origin)?;
                writeln!(
                    out.body,
                    "    reg(clk) {snap}: uint<{span}> reset(rst: 0) initial(0) = if rd && rd_word == {first} {{ {packed} }} else {{ {snap} }};"
                )
                .unwrap();
                for i in 1..k {
                    writeln!(out.reads, "        {} => {},", first + i, word_of(&snap, span, i * 32)).unwrap();
                }
            }
        }
    }
    Ok(out)
}

/// A queue: `depth` slots in a memory, a free-running head and tail, and the
/// counter and entry words at the offsets in [`facet_hdl::queue`]. A queue of
/// `()` has no memory and no data port, just the handshake; its host pushes
/// by writing the new head.
fn queue_port(p: &PortDecl, depth: u32, scope: &mut Scope) -> Result<Parts, NameError> {
    let mut out = Parts::default();
    let n = scope.claim(p.name.into(), "a port")?;
    let origin = format!("internals of queue `{n}`");
    let [valid, ready] = ["valid", "ready"].map(|s| format!("{n}_{s}"));
    let valid = scope.claim(valid, "the valid flag of a queue")?;
    let ready = scope.claim(ready, "the ready flag of a queue")?;
    let [head, tail, room, push, pop, rdp, wrp, entry] =
        ["head", "tail", "room", "push", "pop", "rdp", "wrp", "entry"].map(|s| format!("{n}_{s}"));
    for i in [&head, &tail, &room, &push, &pop, &rdp, &wrp, &entry] {
        scope.claim(i.clone(), &origin)?;
    }
    let (ty, w, e) = (spade_type(&p.ty), p.ty.width(), p.value_words());
    let (tail_w, head_w, entry_w) = (p.word + queue::TAIL, p.word + queue::HEAD, p.word + queue::ENTRY);
    let last = p.word + p.words - 1;

    writeln!(
        out.body,
        "    decl {head}, {tail};\n    let {room} = {head}.wrapping_sub({tail}) < {depth};"
    )
    .unwrap();
    for (word, counter) in [(tail_w, &tail), (head_w, &head)] {
        writeln!(out.reads, "        {word} => {counter},").unwrap();
    }
    let memory = |out: &mut Parts, write_value: &str| {
        writeln!(
            out.body,
            "    memory: [uint<{w}>; {depth}] {{\n        {rdp}: std::mem::Ro<_, _, 0>,\n        {wrp}: std::mem::Wo<_, _, 1>,\n    }};\n    {wrp}.write(clk, if {push} {{ Some((trunc({head}), {write_value})) }} else {{ None }});"
        )
        .unwrap();
    };

    match p.direction {
        Direction::ToFabric => {
            let data_port = if w > 0 {
                format!("    {n}: inv {ty},\n")
            } else {
                String::new()
            };
            writeln!(
                out.ports,
                "    // Host -> fabric queue of {depth}, words {}..={last}; consumed when valid and ready\n{data_port}    {valid}: inv bool,\n    {ready}: bool,",
                p.word
            )
            .unwrap();
            if w == 0 {
                // Only the next count is a push, so a retried write can't add a second.
                writeln!(
                    out.body,
                    "    let {push} = wr && wr_word == {head_w} && wr_data == {head}.wrapping_add(1) && {room};"
                )
                .unwrap();
            } else {
                let mut words = Vec::new();
                for i in 0..e - 1 {
                    let s = scope.claim(format!("{n}_s{i}"), &origin)?;
                    writeln!(
                        out.body,
                        "    reg(clk) {s}: uint<32> reset(rst: 0) initial(0) = if wr && wr_word == {} {{ wr_data }} else {{ {s} }};",
                        entry_w + i
                    )
                    .unwrap();
                    words.push(s);
                }
                words.push("wr_data".into());
                // A push into a full ring is dropped, not an error: the host
                // checks room first, and notices a dropped push as a desync.
                writeln!(out.body, "    let {push} = wr && wr_word == {last} && {room};").unwrap();
                memory(&mut out, &assemble(&words, w));
                writeln!(
                    out.body,
                    "    let {entry}: uint<{w}> = {rdp}.read_comb(Some(trunc({tail}))).unwrap_or(0);\n    set {n} = {};",
                    unpack(&entry, &p.ty)
                )
                .unwrap();
            }
            writeln!(
                out.body,
                "    set {valid} = {head} != {tail};\n    let {pop} = {head} != {tail} && {ready};\n    reg(clk) {head}: uint<32> reset(rst: 0) initial(0) = if {push} {{ {head}.wrapping_add(1) }} else {{ {head} }};\n    reg(clk) {tail}: uint<32> reset(rst: 0) initial(0) = if {pop} {{ {tail}.wrapping_add(1) }} else {{ {tail} }};"
            )
            .unwrap();
        }
        Direction::FromFabric => {
            let data_port = if w > 0 {
                format!("    {n}: {ty},\n")
            } else {
                String::new()
            };
            writeln!(
                out.ports,
                "    // Fabric -> host queue of {depth}, words {}..={last}; pushed when valid and ready\n{data_port}    {valid}: bool,\n    {ready}: inv bool,",
                p.word
            )
            .unwrap();
            // Not ready in reset: the push would be ignored, and a producer
            // that isn't reset with us would count it as taken and lose it.
            writeln!(
                out.body,
                "    let {ready}_now = !rst && {room};\n    set {ready} = {ready}_now;\n    let {push} = {valid} && {ready}_now;\n    // Pop = write the new absolute tail. Anything that isn't a step forward\n    // within what's been produced is ignored, so a retried write can't\n    // consume a second entry.\n    let {pop} = wr && wr_word == {tail_w} && wr_data.wrapping_sub({tail}) <= {head}.wrapping_sub({tail});\n    reg(clk) {head}: uint<32> reset(rst: 0) initial(0) = if {push} {{ {head}.wrapping_add(1) }} else {{ {head} }};\n    reg(clk) {tail}: uint<32> reset(rst: 0) initial(0) = if {pop} {{ wr_data }} else {{ {tail} }};"
            )
            .unwrap();
            scope.claim(format!("{ready}_now"), &origin)?;
            if w > 0 {
                memory(&mut out, &pack(&n, &p.ty));
                let span = e * 32;
                writeln!(
                    out.body,
                    "    let {entry}: uint<{span}> = {};",
                    widen(&format!("{rdp}.read_comb(Some(trunc({tail}))).unwrap_or(0)"), w, span)
                )
                .unwrap();
                writeln!(out.reads, "        {entry_w} => {},", word_of(&entry, span, 0)).unwrap();
                if e > 1 {
                    let snap = scope.claim(format!("{n}_snap"), &origin)?;
                    writeln!(
                        out.body,
                        "    reg(clk) {snap}: uint<{span}> reset(rst: 0) initial(0) = if rd && rd_word == {entry_w} {{ {entry} }} else {{ {snap} }};"
                    )
                    .unwrap();
                    for i in 1..e {
                        writeln!(
                            out.reads,
                            "        {} => {},",
                            entry_w + i,
                            word_of(&snap, span, i * 32)
                        )
                        .unwrap();
                    }
                }
            }
            out.irq.push(format!("{head} != {tail}"));
        }
    }
    Ok(out)
}
