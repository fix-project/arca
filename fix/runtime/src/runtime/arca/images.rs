use super::*;
use alloc::collections::VecDeque;
use core::cell::RefCell;

const CAPACITY: usize = 16;

struct Image {
    runtime: Option<usize>,
    executable: Blob,
    function: Function,
}

#[kernel::core_local]
static IMAGES: RefCell<VecDeque<Image>> = RefCell::new(VecDeque::new());

impl<S: Storage + AsRef<MemoryStorage>> Store<S> {
    pub(super) fn image(&self, executable: Blob) -> Function {
        let runtime = (!executable.as_handle().is_canonical()).then_some(self.image_cache_id);
        let cached = {
            let mut images = IMAGES.borrow_mut();
            images
                .iter()
                .position(|image| {
                    image.runtime == runtime
                        && image
                            .executable
                            .as_handle()
                            .same_encoding(executable.as_handle())
                })
                .and_then(|index| images.remove(index))
        };
        let image = cached.unwrap_or_else(|| {
            let gate = self
                .images
                .lock()
                .entry(*executable.as_handle().as_bytes())
                .or_insert_with(|| Arc::new(KMutex::new(None)))
                .clone();
            let function = gate
                .lock()
                .get_or_insert_with(|| {
                    let elf = self.get_blob_backing(executable).unwrap();
                    let started = self.profile.load(Ordering::Relaxed).then(kernel::tsc::read);
                    let function =
                        common::elfloader::load_elf_from_pages(elf.clone().into_buf(), |offset| {
                            let (page, within) = elf.page(offset);
                            (within == 0 && page.len() == 4096).then_some(page)
                        })
                        .unwrap();
                    self.record_elf_load(started);
                    function
                })
                .clone();
            Image {
                runtime,
                executable,
                function,
            }
        });
        let function = image.function.clone();
        let evicted = {
            let mut images = IMAGES.borrow_mut();
            let evicted = if images.len() == CAPACITY {
                images.pop_front()
            } else {
                None
            };
            images.push_back(image);
            evicted
        };
        drop(evicted);
        function
    }
}
