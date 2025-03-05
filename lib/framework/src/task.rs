use std::error::Error;
use std::future::Future;
use std::sync::LazyLock;

use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;
use tracing::Instrument;
use tracing::Span;
use tracing::info;

static TASK_TRACKER: LazyLock<TaskTracker> = LazyLock::new(TaskTracker::new);

pub fn spawn<F>(task: F) -> JoinHandle<()>
where
    F: Future<Output = Result<(), Box<dyn Error>>> + Send + 'static,
{
    let span = Span::current();

    let task_wrapper = async move {
        let result = task.await;
        if let Err(err) = result {
            panic!("task failed, error={}", err);
        }
    };

    TASK_TRACKER.spawn(task_wrapper.instrument(span))
}

pub async fn shutdown() {
    info!("waiting for {} task(s) to finish", TASK_TRACKER.len());
    TASK_TRACKER.close();
    TASK_TRACKER.wait().await;
    info!("tasks finished");
}
