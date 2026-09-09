use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use chrono::Utc;
use orangedeck_domain::{JobEvent, JobKind, JobRecord, JobStatus, OutputStream, Project};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{Mutex, Semaphore, broadcast},
    task::JoinHandle,
    time::{Duration, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

#[derive(Clone)]
pub struct CargoJobRunner {
    cargo_binary: PathBuf,
    active: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
    slots: Arc<Semaphore>,
    events: broadcast::Sender<JobEvent>,
}

impl CargoJobRunner {
    pub fn new(cargo_binary: impl Into<PathBuf>) -> Self {
        let (events, _) = broadcast::channel(512);
        Self {
            cargo_binary: cargo_binary.into(),
            active: Arc::new(Mutex::new(HashMap::new())),
            slots: Arc::new(Semaphore::new(1)),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.events.subscribe()
    }

    pub async fn start(&self, project: &Project, kind: JobKind) -> Result<Uuid, JobError> {
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| JobError::Busy)?;
        let id = Uuid::new_v4();
        let mut command = Command::new(&self.cargo_binary);
        command
            .args(kind.cargo_args())
            .current_dir(&project.path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(JobError::Spawn)?;
        let stdout = child.stdout.take().ok_or(JobError::MissingPipe("stdout"))?;
        let stderr = child.stderr.take().ok_or(JobError::MissingPipe("stderr"))?;
        let cancellation = CancellationToken::new();
        self.active.lock().await.insert(id, cancellation.clone());

        let mut record = JobRecord::queued(id, kind, project.id.clone());
        record.start(Utc::now()).map_err(JobError::Transition)?;
        let _ = self.events.send(JobEvent::Started(record.clone()));

        let runner = self.clone();
        let sequence = Arc::new(AtomicU64::new(0));
        let stdout_task = stream_lines(
            stdout,
            id,
            OutputStream::Stdout,
            sequence.clone(),
            self.events.clone(),
        );
        let stderr_task = stream_lines(
            stderr,
            id,
            OutputStream::Stderr,
            sequence,
            self.events.clone(),
        );
        tokio::spawn(async move {
            let _permit = permit;
            let started = Instant::now();
            let outcome = tokio::select! {
                status = child.wait() => match status {
                    Ok(status) if status.success() => (JobStatus::Succeeded, status.code()),
                    Ok(status) => (JobStatus::Failed, status.code()),
                    Err(error) => {
                        warn!(job_id = %id, %error, "cargo job wait failed");
                        (JobStatus::Failed, None)
                    }
                },
                () = cancellation.cancelled() => {
                    terminate_process_tree(&mut child).await;
                    (JobStatus::Cancelled, None)
                }
            };
            await_reader(stdout_task, id).await;
            await_reader(stderr_task, id).await;
            let duration = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            if let Err(error) = record.finish(outcome.0, Utc::now(), outcome.1, duration) {
                warn!(job_id = %id, %error, "cargo job state transition failed");
            }
            runner.active.lock().await.remove(&id);
            let _ = runner.events.send(JobEvent::Completed(record));
        });

        Ok(id)
    }

    pub async fn cancel(&self, id: Uuid) -> bool {
        let active = self.active.lock().await;
        if let Some(cancellation) = active.get(&id) {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    pub async fn active_jobs(&self) -> Vec<Uuid> {
        self.active.lock().await.keys().copied().collect()
    }

    pub async fn shutdown(&self) {
        let cancellations = self
            .active
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for cancellation in cancellations {
            cancellation.cancel();
        }
        if timeout(Duration::from_secs(4), async {
            while !self.active.lock().await.is_empty() {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .is_err()
        {
            warn!("timed out while stopping Cargo jobs");
        }
    }
}

fn stream_lines<R>(
    reader: R,
    job_id: Uuid,
    stream: OutputStream,
    sequence: Arc<AtomicU64>,
    sender: broadcast::Sender<JobEvent>,
) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    let sequence = sequence.fetch_add(1, Ordering::Relaxed);
                    let _ = sender.send(JobEvent::Output {
                        job_id,
                        stream,
                        sequence,
                        line: truncate_output_line(&line),
                    });
                }
                Ok(None) => break,
                Err(error) => {
                    warn!(job_id = %job_id, %error, "cannot read cargo output");
                    break;
                }
            }
        }
    })
}

fn truncate_output_line(line: &str) -> String {
    const MAX_LINE_CHARS: usize = 4_096;
    let mut chars = line.chars();
    let prefix = chars.by_ref().take(MAX_LINE_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

async fn await_reader(task: JoinHandle<()>, job_id: Uuid) {
    if let Err(error) = task.await {
        warn!(job_id = %job_id, %error, "cargo output reader stopped unexpectedly");
    }
}

#[cfg(unix)]
async fn terminate_process_tree(child: &mut Child) {
    use nix::{
        sys::signal::{Signal, killpg},
        unistd::Pid,
    };

    if let Some(raw_id) = child.id()
        && let Ok(id) = i32::try_from(raw_id)
    {
        let _ = killpg(Pid::from_raw(id), Signal::SIGTERM);
    }
    if timeout(Duration::from_secs(2), child.wait()).await.is_err() {
        if let Some(raw_id) = child.id()
            && let Ok(id) = i32::try_from(raw_id)
        {
            let _ = killpg(Pid::from_raw(id), Signal::SIGKILL);
        }
        let _ = child.wait().await;
    }
}

#[cfg(not(unix))]
async fn terminate_process_tree(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[derive(Debug, Error)]
pub enum JobError {
    #[error("another Cargo job is already running")]
    Busy,
    #[error("cannot start cargo: {0}")]
    Spawn(std::io::Error),
    #[error("cargo process did not expose {0}")]
    MissingPipe(&'static str),
    #[error(transparent)]
    Transition(orangedeck_domain::JobTransitionError),
}
