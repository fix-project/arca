#[cfg(not(target_os = "linux"))]
pub use kernel::kthread::KMutex as Mutex;

#[cfg(target_os = "linux")]
#[derive(Debug, Default)]
pub struct Mutex<T>(std::sync::Mutex<T>);

#[cfg(target_os = "linux")]
impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, T> {
        self.0.lock().unwrap()
    }
}
