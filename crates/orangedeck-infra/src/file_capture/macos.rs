//! One FSEvents stream per running tool. Registration never enumerates the tree.
//! All native objects and callbacks stay on their owning worker thread. Start is
//! acknowledged only after success; finish flushes pending events before release.
use super::{EventFlags, Events};
use objc2_core_foundation::{CFArray, CFRunLoop, CFString, kCFRunLoopDefaultMode};
use objc2_core_services as fs;
use std::{
    ffi::{CStr, c_char, c_void},
    io,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(2);
static WORKERS: AtomicUsize = AtomicUsize::new(0);
struct Permit;
impl Permit {
    fn acquire() -> io::Result<Self> {
        WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < 4).then_some(count + 1)
            })
            .map(|_| Self)
            .map_err(|_| io::Error::other("file event observer capacity reached"))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) struct Watcher {
    finish: SyncSender<()>,
    result: Receiver<io::Result<Events>>,
}

impl Watcher {
    pub(super) fn start(root: &Path) -> io::Result<Self> {
        let permit = Permit::acquire()?;
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (finish_tx, finish_rx) = mpsc::sync_channel(1);
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let root = root.to_owned();
        thread::Builder::new()
            .name("file-events".into())
            .spawn(move || {
                let _permit = permit;
                let result = observe(root, &ready_tx, &finish_rx);
                if let Err(error) = &result {
                    let _ = ready_tx.try_send(Err(io::Error::other(error.to_string())));
                }
                let _ = result_tx.send(result);
            })?;
        ready_rx
            .recv_timeout(TIMEOUT)
            .map_err(|_| io::Error::other("file event observer did not start"))??;
        Ok(Self {
            finish: finish_tx,
            result: result_rx,
        })
    }

    pub(super) fn finish(self) -> io::Result<Events> {
        self.finish
            .send(())
            .map_err(|_| io::Error::other("file event observer stopped"))?;
        self.result
            .recv_timeout(TIMEOUT)
            .map_err(|_| io::Error::other("file event flush did not complete"))?
    }
}

// Not Send/Sync: this owner never leaves the thread which created the stream.
struct Stream {
    raw: fs::FSEventStreamRef,
    started: bool,
}
impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: raw is a non-null create-owned stream. All callbacks run on this
        // thread; invalidation precedes release and the callback context outlives it.
        unsafe {
            if self.started {
                fs::FSEventStreamStop(self.raw);
            }
            fs::FSEventStreamInvalidate(self.raw);
            fs::FSEventStreamRelease(self.raw);
        }
    }
}

fn observe(
    root: PathBuf,
    ready: &SyncSender<io::Result<()>>,
    finish: &Receiver<()>,
) -> io::Result<Events> {
    let name = root
        .to_str()
        .ok_or_else(|| io::Error::other("non-UTF-8 working folder"))?;
    let paths = CFArray::from_retained_objects(&[CFString::from_str(name)]);
    let run_loop =
        CFRunLoop::current().ok_or_else(|| io::Error::other("no file event run loop"))?;
    // SAFETY: Apple's process-lifetime immutable default run-loop mode.
    let mode =
        unsafe { kCFRunLoopDefaultMode }.ok_or_else(|| io::Error::other("no run loop mode"))?;
    // Box address stays fixed. Only the callback accesses it while native calls run.
    let mut events = Box::new(Events::new(root.clone()));
    let mut context = fs::FSEventStreamContext {
        version: 0,
        info: (&raw mut *events).cast(),
        retain: None,
        release: None,
        copyDescription: None,
    };
    // SAFETY: CFArray contains CFStrings; context and its Box remain alive until
    // Stream drops. No CFTypes flag, so the callback receives a char** path array.
    let raw = unsafe {
        fs::FSEventStreamCreate(
            None,
            Some(callback),
            &raw mut context,
            paths.as_opaque(),
            fs::kFSEventStreamEventIdSinceNow,
            0.0,
            fs::kFSEventStreamCreateFlagFileEvents
                | fs::kFSEventStreamCreateFlagNoDefer
                | fs::kFSEventStreamCreateFlagWatchRoot,
        )
    };
    if raw.is_null() {
        return Err(io::Error::other("could not create file event stream"));
    }
    let mut stream = Stream {
        raw,
        started: false,
    };
    // SAFETY: stream is owned here and scheduled only on this thread's run loop.
    // Run-loop delivery permits synchronous flush on that same thread.
    #[allow(deprecated)]
    unsafe {
        fs::FSEventStreamScheduleWithRunLoop(stream.raw, &run_loop, mode);
    }
    // SAFETY: valid, scheduled stream, not yet started.
    stream.started = unsafe { fs::FSEventStreamStart(stream.raw) };
    if !stream.started {
        return Err(io::Error::other("could not start file event stream"));
    }
    // Discard events preceding the PreToolUse acknowledgement. FlushSync runs its
    // private run-loop mode and completes every pending callback before returning.
    // SAFETY: started stream; no Events reference is borrowed during callbacks.
    unsafe {
        fs::FSEventStreamFlushSync(stream.raw);
    }
    *events = Events::new(root);
    ready
        .send(Ok(()))
        .map_err(|_| io::Error::other("file event start cancelled"))?;
    loop {
        match finish.try_recv() {
            Ok(()) => {
                // SAFETY: as above. This is an OS delivery barrier, not a sleep or scan.
                unsafe {
                    fs::FSEventStreamFlushSync(stream.raw);
                }
                drop(stream);
                return Ok(*events);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(io::Error::other("file observation cancelled"));
            }
            Err(mpsc::TryRecvError::Empty) => {
                CFRunLoop::run_in_mode(Some(mode), 0.02, true);
            }
        }
    }
}

unsafe extern "C-unwind" fn callback(
    _stream: fs::ConstFSEventStreamRef,
    info: *mut c_void,
    count: usize,
    paths: NonNull<c_void>,
    flags: NonNull<fs::FSEventStreamEventFlags>,
    _ids: NonNull<fs::FSEventStreamEventId>,
) {
    // SAFETY: info is our live Box; FSEvents invokes this only on its owning run
    // loop thread. Native arrays have count elements and are valid for this call.
    let events = unsafe { &mut *info.cast::<Events>() };
    if count > 65_536 {
        events.incomplete = true;
        return;
    }
    let paths =
        unsafe { std::slice::from_raw_parts(paths.as_ptr().cast::<*const c_char>(), count) };
    let flags = unsafe { std::slice::from_raw_parts(flags.as_ptr(), count) };
    for (&path, &flags) in paths.iter().zip(flags) {
        let lost = fs::kFSEventStreamEventFlagMustScanSubDirs
            | fs::kFSEventStreamEventFlagUserDropped
            | fs::kFSEventStreamEventFlagKernelDropped
            | fs::kFSEventStreamEventFlagEventIdsWrapped
            | fs::kFSEventStreamEventFlagRootChanged
            | fs::kFSEventStreamEventFlagUnmount;
        events.incomplete |= flags & lost != 0;
        if flags & fs::kFSEventStreamEventFlagHistoryDone != 0 {
            continue;
        }
        if path.is_null() {
            events.incomplete = true;
            continue;
        }
        // SAFETY: each native path is a NUL-terminated string valid in this callback.
        let Ok(path) = (unsafe { CStr::from_ptr(path) }).to_str() else {
            events.incomplete = true;
            continue;
        };
        let path = Path::new(path);
        if flags & fs::kFSEventStreamEventFlagItemIsDir != 0 {
            // Moving/removing a populated folder may omit individual file events.
            // Report that gap; never fall back to walking its contents.
            if events.accepts(path)
                && flags
                    & (fs::kFSEventStreamEventFlagItemRenamed
                        | fs::kFSEventStreamEventFlagItemRemoved)
                    != 0
            {
                events.incomplete = true;
            }
            continue;
        }
        if flags & fs::kFSEventStreamEventFlagItemIsSymlink != 0 {
            continue;
        }
        let changed = fs::kFSEventStreamEventFlagItemCreated
            | fs::kFSEventStreamEventFlagItemRemoved
            | fs::kFSEventStreamEventFlagItemRenamed
            | fs::kFSEventStreamEventFlagItemModified
            | fs::kFSEventStreamEventFlagItemCloned;
        if flags & fs::kFSEventStreamEventFlagItemIsFile != 0 && flags & changed != 0 {
            events.record(
                path,
                EventFlags {
                    created: flags & fs::kFSEventStreamEventFlagItemCreated != 0,
                    removed: flags & fs::kFSEventStreamEventFlagItemRemoved != 0,
                    renamed: flags & fs::kFSEventStreamEventFlagItemRenamed != 0,
                },
            );
        }
    }
}
