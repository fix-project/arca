#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use vmm::client::*;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let arca = vmm::client::runtime();

    log::info!("create thunk");
    let thunk: Ref<Thunk> = arca.load_elf(MODULE);
    log::info!("run thunk");
    let lambda: Ref<Lambda> = thunk.run().try_into().unwrap();

    let mut tree = arca.create_tree(4);
    let dummy = arca.create_word(0xcafeb0ba);
    tree.put(0, dummy.clone().into());
    tree.put(1, dummy.into());
    tree.put(2, arca.create_word(7).into());
    tree.put(3, arca.create_word(1024).into());

    let thunk = lambda.apply(tree);
    let word: Ref<Word> = thunk.run().try_into().unwrap();
    log::info!("{:?}", word.read());
    assert_eq!(word.read(), 1031);

    Ok(())
}

#[cfg(test)]
mod test {
    use vmm::client::*;
    const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));

    #[test]
    fn test_addblob() {
        let arca = vmm::client::runtime();
        let thunk: Ref<Thunk> = arca.load_elf(MODULE);
        let lambda: Ref<Lambda> = thunk.run().try_into().unwrap();

        let mut tree = arca.create_tree(4);
        let dummy = arca.create_word(0xcafeb0ba);
        tree.put(0, dummy.clone().into());
        tree.put(1, dummy.into());
        tree.put(2, arca.create_word(7).into());
        tree.put(3, arca.create_word(1024).into());

        let thunk = lambda.apply(tree);
        let word: Ref<Word> = thunk.run().try_into().unwrap();
        assert_eq!(word.read(), 1031);
    }
}
