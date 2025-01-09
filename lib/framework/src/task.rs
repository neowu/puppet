use std::future::Future;
use std::sync::LazyLock;

use tokio::task::JoinHandle;
use tokio_util::task::TaskTracker;
use tracing::info;
use tracing::Instrument;
use tracing::Span;

static TASK_TRACKER: LazyLock<TaskTracker> = LazyLock::new(TaskTracker::new);

pub fn spawn<F>(task: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let span = Span::current();
    TASK_TRACKER.spawn(task.instrument(span))
}

pub async fn shutdown() {
    info!("waiting for {} task(s) to finish", TASK_TRACKER.len());
    TASK_TRACKER.close();
    TASK_TRACKER.wait().await;
    info!("tasks finished");
}
