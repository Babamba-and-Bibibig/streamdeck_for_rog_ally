//! Linux development fixtures use one nonrecursive inotify watch; no tree walk.
//! Recursive capture is a macOS Connector feature. Other platforms report incomplete.
use super::Events;
use std::{io, path::Path};

#[cfg(target_os = "linux")]
use super::EventFlags;
#[cfg(target_os = "linux")]
use nix::sys::inotify::{AddWatchFlags as Flags, InitFlags, Inotify};

pub(super) struct Watcher {
    events: Events,
    #[cfg(target_os = "linux")]
    watcher: Inotify,
}

impl Watcher {
    pub(super) fn start(path: &Path) -> io::Result<Self> {
        let mut events = Events::new(path.to_owned());
        events.incomplete = true;
        #[cfg(target_os = "linux")]
        {
            let watcher = Inotify::init(InitFlags::IN_CLOEXEC | InitFlags::IN_NONBLOCK)?;
            watcher.add_watch(
                path,
                Flags::IN_MODIFY
                    | Flags::IN_CREATE
                    | Flags::IN_DELETE
                    | Flags::IN_MOVED_FROM
                    | Flags::IN_MOVED_TO
                    | Flags::IN_DELETE_SELF
                    | Flags::IN_MOVE_SELF
                    | Flags::IN_ONLYDIR
                    | Flags::IN_DONT_FOLLOW,
            )?;
            Ok(Self { events, watcher })
        }
        #[cfg(not(target_os = "linux"))]
        Ok(Self { events })
    }

    pub(super) fn finish(mut self) -> io::Result<Events> {
        #[cfg(target_os = "linux")]
        let mut received = 0;
        #[cfg(target_os = "linux")]
        loop {
            match self.watcher.read_events() {
                Ok(events) => {
                    for event in events {
                        received += 1;
                        if received > 65_536 {
                            return Ok(self.events);
                        }
                        if event.mask.intersects(
                            Flags::IN_Q_OVERFLOW
                                | Flags::IN_IGNORED
                                | Flags::IN_UNMOUNT
                                | Flags::IN_DELETE_SELF
                                | Flags::IN_MOVE_SELF,
                        ) {
                            self.events.incomplete = true;
                        }
                        if event.mask.contains(Flags::IN_ISDIR) {
                            continue;
                        }
                        if let Some(name) = event.name {
                            let path = self.events.root.join(name);
                            self.events.record(
                                &path,
                                EventFlags {
                                    created: event.mask.contains(Flags::IN_CREATE),
                                    removed: event
                                        .mask
                                        .intersects(Flags::IN_DELETE | Flags::IN_MOVED_FROM),
                                    renamed: event
                                        .mask
                                        .intersects(Flags::IN_MOVED_FROM | Flags::IN_MOVED_TO),
                                },
                            );
                        }
                    }
                }
                Err(nix::errno::Errno::EAGAIN) => break,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(self.events)
    }
}
