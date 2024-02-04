use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::DefaultMemoryImpl;
use std::cell::RefCell;

// A memory for the StableBTreeMap we're using. A new memory should be created for
// every additional stable structure.
const SEEDS: MemoryId = MemoryId::new(1);

const SIG_COUNT: MemoryId = MemoryId::new(2);

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
}

pub fn get_seeds() -> Memory {
    MEMORY_MANAGER.with(|m| m.borrow().get(SEEDS))
}

pub fn get_sig_count() -> Memory {
    MEMORY_MANAGER.with(|m| m.borrow().get(SIG_COUNT))
}