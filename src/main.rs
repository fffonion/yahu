mod backend;

const TOKIO_WORKER_THREADS: usize = 4;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(TOKIO_WORKER_THREADS)
        .build()?
        .block_on(backend::run())
}

#[cfg(test)]
mod runtime_tests {
    use super::TOKIO_WORKER_THREADS;

    #[test]
    fn tokio_worker_pool_is_capped_for_web_ui_workload() {
        assert_eq!(TOKIO_WORKER_THREADS, 4);
    }
}
