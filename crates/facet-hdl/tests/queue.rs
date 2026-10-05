//! Queue-end bookkeeping against plain memory. There's no fabric behind it,
//! so the counters only move when a test moves them: that's what makes the
//! protocol violations below reproducible. Real push/pop traffic is covered
//! by the GHDL co-sim in facet-hdl-ghdl.

mod common;

use common::Mem;
use facet::Facet;
use facet_hdl::{BindError, FromFabricQueue, PortError, PushError, ToFabricQueue, bind, queue};

#[derive(Facet)]
pub struct Q {
    pub jobs: ToFabricQueue<u64, 2>,
    pub done: FromFabricQueue<u32, 2>,
}

#[test]
fn a_full_ring_refuses_the_push_without_writing() {
    let (mem, b) = Mem::fresh::<Q>();
    let mut q: Q = bind(mem.clone()).unwrap();
    let jobs = &b.ports[0];
    // A fabric that consumed nothing but counted both pushes.
    q.jobs.push(&1).unwrap();
    q.jobs.push(&2).unwrap();
    mem.set(jobs.word + queue::HEAD, 2);
    mem.set(jobs.word + queue::ENTRY, 0xAAAA);
    assert!(matches!(q.jobs.push(&3), Err(PushError::Full)));
    assert_eq!(
        mem.get(jobs.word + queue::ENTRY),
        0xAAAA,
        "a refused push touches nothing"
    );
}

#[test]
fn a_fabric_head_that_disagrees_with_ours_is_a_desync() {
    let (mem, _) = Mem::fresh::<Q>();
    let mut q: Q = bind(mem).unwrap();
    // Nothing behind the window ever advances head, as if a second
    // producer's pushes landed and ours didn't.
    q.jobs.push(&1).unwrap();
    q.jobs.push(&2).unwrap();
    let err = q.jobs.push(&3).unwrap_err();
    assert!(
        matches!(err, PushError::Port(PortError::Desync { port: "jobs", .. })),
        "{err}"
    );
}

#[test]
fn a_count_gap_wider_than_the_ring_is_never_used_as_an_index() {
    let (mem, b) = Mem::fresh::<Q>();
    let mut q: Q = bind(mem.clone()).unwrap();
    mem.set(b.ports[1].word + queue::HEAD, 7);
    assert!(matches!(q.done.pop(), Err(PortError::Desync { port: "done", .. })));
}

#[test]
fn bind_resumes_from_the_fabric_counters_and_rejects_bad_ones() {
    let (mem, b) = Mem::fresh::<Q>();
    let done = &b.ports[1];
    // A previous host popped 5 of 6.
    mem.set(done.word + queue::HEAD, 6);
    mem.set(done.word + queue::TAIL, 5);
    mem.set(done.word + queue::ENTRY, 42);
    {
        let mut q: Q = bind(mem.clone()).unwrap();
        assert_eq!(q.done.pop().unwrap(), Some(42));
        assert_eq!(mem.get(done.word + queue::TAIL), 6, "pop writes the absolute new tail");
        assert_eq!(q.done.pop().unwrap(), None);
    }
    mem.set(done.word + queue::HEAD, 100);
    assert!(matches!(bind::<Q>(mem), Err(BindError::Port(PortError::Desync { .. }))));
}
