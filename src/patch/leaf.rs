use core::sync::atomic;
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use siphasher::sip128::{Hasher128, SipHasher24};
use std::alloc::*;

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

    pub(crate) unsafe fn put<O: KeyOrdering<KEY_LEN>, S: KeySegmentation<KEY_LEN>>(
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
                    let new_branch = Branch2::new(depth);
                    Branch2::insert(new_branch, entry.leaf(depth), entry.hash);
                    Branch2::insert(new_branch, head.with_start(depth), head.hash());

                    *head = Head::new(HeadTag::Branch2, head.key().unwrap(), new_branch);
                    return;
                }
            }
        }
    }

    pub(crate) unsafe fn infixes<
        const INFIX_LEN: usize,
        O: KeyOrdering<KEY_LEN>,
        S: KeySegmentation<KEY_LEN>,
        F,
    >(
        node: *const Self,
        key: &[u8; KEY_LEN],
        at_depth: usize,
        start_depth: usize,
        f: F,
        out: &mut Vec<[u8; INFIX_LEN]>,
    ) where
        F: Fn([u8; KEY_LEN]) -> [u8; INFIX_LEN],
    {
        let leaf_key = &(*node).key;
        for depth in at_depth..start_depth {
            let key_depth = O::key_index(depth);
            if leaf_key[key_depth] != key[key_depth] {
                return;
            }
        }
        unsafe {
            out.push(f((*node).key));
        }
    }

    pub(crate) unsafe fn has_prefix<O: KeyOrdering<KEY_LEN>>(
        node: *const Self,
        at_depth: usize,
        key: &[u8; KEY_LEN],
        end_depth: usize,
    ) -> bool {
        let leaf_key: &[u8; KEY_LEN] = &(*node).key;
        for depth in at_depth..=end_depth {
            let key_depth = O::key_index(depth);
            if leaf_key[key_depth] != key[key_depth] {
                return false;
            }
        }
        return true;
    }

    pub(crate) unsafe fn segmented_len<O: KeyOrdering<KEY_LEN>>(
        node: *const Self,
        at_depth: usize,
        key: &[u8; KEY_LEN],
        start_depth: usize,
    ) -> usize {
        let leaf_key: &[u8; KEY_LEN] = &(*node).key;
        for depth in at_depth..start_depth {
            let key_depth = O::key_index(depth);
            if leaf_key[key_depth] != key[key_depth] {
                return 0;
            }
        }
        return 1;
    }
}
