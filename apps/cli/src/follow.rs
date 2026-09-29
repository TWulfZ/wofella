//! Following one started job: progress on stderr, cancel on the first interrupt, then its
//! history entry (spec 005 Behaviour, "CLI"). Shared by `sync` and `library index`.

use std::future::Future;
use std::pin::Pin;

use tokio::sync::broadcast::{Receiver, error::RecvError};
use wolluf_app::context::AppContext;
use wolluf_app::errors::AppError;
use wolluf_app::events::AppEvent;
use wolluf_app::jobs::dto::{JobDto, JobId};
use wolluf_core::ErrorCode;

use crate::progress::Progress;

/// Resolves on the first Ctrl-C. If the handler cannot be installed the job simply runs
/// uninterruptible instead of being cancelled at once.
pub(crate) async fn ctrl_c() {
    if tokio::signal::ctrl_c().await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// Waits for `id` to finish and returns whether `interrupt` fired. `rx` must be subscribed
/// before the job was started, or its finish event can slip past.
pub(crate) async fn until_finished<F: Future<Output = ()>>(
    ctx: &AppContext,
    rx: &mut Receiver<AppEvent>,
    id: &JobId,
    progress: &mut Progress,
    mut interrupt: Pin<&mut F>,
) -> Result<bool, AppError> {
    let mut interrupted = false;
    loop {
        tokio::select! {
            biased;
            () = &mut interrupt, if !interrupted => {
                interrupted = true;
                match ctx.job_service().cancel(id).await {
                    // NOT_FOUND: the job already ended and its finish event is on the bus.
                    Ok(()) => {}
                    Err(e) if e.code == ErrorCode::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
            event = rx.recv() => match event {
                Ok(AppEvent::JobProgress(p)) if &p.job_id == id => progress.update(&p),
                Ok(AppEvent::JobFinished(f)) if &f.job_id == id => break,
                // A lagging receiver only lost progress; the history is authoritative.
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(AppError::internal("event bus closed while waiting for a job"));
                }
            },
        }
    }
    progress.finish();
    Ok(interrupted)
}

pub(crate) async fn history_entry(ctx: &AppContext, id: &JobId) -> Result<JobDto, AppError> {
    ctx.job_service()
        .list(None)
        .await?
        .into_iter()
        .find(|j| &j.id == id)
        .ok_or_else(|| AppError::internal(format!("job {id} missing from history")))
}
