use std::future::Future;

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_task<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    let _ = smol::spawn(fut);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_task<F>(fut: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(fut);
}
