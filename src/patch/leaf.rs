use core::sync::atomic;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use siphasher::sip128::{Hasher128, SipHasher24};
use std::alloc::*;
use std::convert::TryInto;

//use crate::trible::Value;

use super::*;

#[derive(Debug)]
#[repr(C)]
pub(crate) struct Leaf<const KEY_LEN: usize> {
    pub key: [u8; KEY_LEN],
    rc: atomic::AtomicU32,
}

impl<const KEY_LEN: usize> Leaf<KEY_LEN> {
    pub(super) unsafe fn new(key: &[u8; KEY_LEN]) -> *mut Self {
        unsafe {
            let layout = Layout::new::<Self>();
            let ptr = alloc(layout) as *mut Self;
            if ptr.is_null() {
                panic!("Allocation failed!");
            }
            std::ptr::write(
                ptr,
                Self {
                    key: *key,
                    rc: atomic::AtomicU32::new(1),
                },
            );

            ptr
        }
    }

    pub(crate) unsafe fn rc_inc(node: *mut Self) -> *mut Self {
        unsafe {
            let mut current = (*node).rc.load(Relaxed);
            loop {
                if current == u32::MAX {
                    panic!("max refcount exceeded");
                }
                match (*node)
                    .rc
                    .compare_exchange(current, current + 1, Relaxed, Relaxed)
                {
                    Ok(_) => return node,
                    Err(v) => current = v,
                }
            }
        }
    }

    pub(crate) unsafe fn rc_dec(node: *mut Self) {
        unsafe {
            let rc = (*node).rc.fetch_sub(1, Release);
            if rc != 1 {
                return;
            }
            (*node).rc.load(Acquire);

            std::ptr::drop_in_place(node);

            let layout = Layout::new::<Self>();
            let ptr = node as *mut u8;
            dealloc(ptr, layout);
        }
    }

    pub(crate) unsafe fn peek(node: *const Self, at_depth: usize) -> u8 {
        unsafe { (*node).key[at_depth] }
    }

    pub(crate) unsafe fn hash(node: *const Self) -> u128 {
        unsafe {
            let mut hasher = SipHasher24::new_with_key(&SIP_KEY);
            hasher.write(&(*node).key[..]);
            return hasher.finish128().into();
        }
    }

    pub(crate) unsafe fn insert<O: KeyOrdering<KEY_LEN>, S: KeySegmentation<KEY_LEN>>(
        head: &mut Head<KEY_LEN, O, S>,
        entry: &Entry<KEY_LEN>,
        at_depth: usize,
    ) {
        debug_assert!(head.tag() == HeadTag::Leaf);
        unsafe {
            let node: *const Self = head.ptr();
            let leaf_key: &[u8; KEY_LEN] = &(*node).key;
            for depth in at_depth..KEY_LEN {
                let key_depth = O::key_index(depth);
                if leaf_key[key_depth] != entry.peek(key_depth) {
                    let key = head.key().unwrap();
                    let new_branch = Branch2::new(depth);
                    let new_head = Head::new(HeadTag::Branch2, key, new_branch);
                    let old_head = std::mem::replace(head, new_head);

                    let old_head_hash = old_head.hash();

                    Branch2::insert_child(new_branch, entry.leaf(depth), entry.hash);
                    Branch2::insert_child(new_branch, old_head.with_start(depth), old_head_hash);

                    return;
                }
            }
        }
    }

    pub(crate) unsafe fn infixes<
        const PREFIX_LEN: usize,
        const INFIX_LEN: usize,
        O: KeyOrdering<KEY_LEN>,
        S: KeySegmentation<KEY_LEN>,
        F,
    >(
        node: *const Self,
        prefix: &[u8; PREFIX_LEN],
        at_depth: usize,
        f: &mut F,
    ) where
        F: FnMut([u8; INFIX_LEN]),
    {
        let leaf_key = &(*node).key;
        for depth in at_depth..PREFIX_LEN {
            if leaf_key[O::key_index(depth)] != prefix[depth] {
                return;
            }
        }

        let end_depth = PREFIX_LEN + INFIX_LEN - 1;
        let infix = unsafe {
            (*node).key[O::key_index(PREFIX_LEN)..=O::key_index(end_depth)]
                .try_into()
                .expect("invalid infix range")
        };
        f(infix);
    }

    pub(crate) unsafe fn has_prefix<O: KeyOrdering<KEY_LEN>, const PREFIX_LEN: usize>(
        node: *const Self,
        at_depth: usize,
        prefix: &[u8; PREFIX_LEN],
    ) -> bool {
        let leaf_key: &[u8; KEY_LEN] = &(*node).key;
        for depth in at_depth..PREFIX_LEN {
            if leaf_key[O::key_index(depth)] != prefix[depth] {
                return false;
            }
        }
        return true;
    }

    pub(crate) unsafe fn segmented_len<O: KeyOrdering<KEY_LEN>, const PREFIX_LEN: usize>(
        node: *const Self,
        at_depth: usize,
        prefix: &[u8; PREFIX_LEN],
    ) -> u64 {
        let leaf_key: &[u8; KEY_LEN] = &(*node).key;
        for depth in at_depth..PREFIX_LEN {
            let key_depth = O::key_index(depth);
            if leaf_key[key_depth] != prefix[depth] {
                return 0;
            }
        }
        return 1;
    }
}
