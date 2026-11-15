use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use raw_window_handle::RawDisplayHandle;
use x11rb::CURRENT_TIME;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt, CreateWindowAux, EventMask, GrabMode, GrabStatus, PropMode,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{EventLog, Native, NativeError, NativeEvent, uri};

const XDND_VERSION: u32 = 5;
const ESCAPE_KEYCODE: u8 = 9;
const FINISH_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_WAIT: Duration = Duration::from_millis(5);

type Failure = Box<dyn std::error::Error + Send + Sync>;

pub struct X11Native {
    xlib_display: Option<usize>,
    events: EventLog,
    cancelled: Arc<AtomicBool>,
}

impl std::fmt::Debug for X11Native {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("X11Native")
    }
}

impl X11Native {
    pub fn attach(window: &dyn iced::window::Window) -> Option<X11Native> {
        let xlib_display = match window.display_handle().ok()?.as_raw() {
            RawDisplayHandle::Xlib(handle) => Some(handle.display?.as_ptr() as usize),
            RawDisplayHandle::Xcb(_) => None,
            _ => return None,
        };
        Some(X11Native { xlib_display, events: EventLog::default(), cancelled: Arc::new(AtomicBool::new(false)) })
    }
}

impl Native for X11Native {
    fn start_drag(&mut self, paths: Vec<PathBuf>) -> Result<(), NativeError> {
        if let Some(display) = self.xlib_display {
            release_the_implicit_grab(display);
        }
        self.cancelled.store(false, Ordering::SeqCst);
        let events = self.events.clone();
        let cancelled = self.cancelled.clone();
        std::thread::spawn(move || {
            let outcome = run_drag(&paths, &events, &cancelled).unwrap_or_else(|error| {
                tracing::warn!("the X11 drag failed: {error}");
                NativeEvent::Cancelled
            });
            events.push(outcome);
        });
        Ok(())
    }

    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn poll(&mut self) -> Vec<NativeEvent> {
        self.events.take()
    }
}

fn release_the_implicit_grab(display: usize) {
    if let Ok(xlib) = x11_dl::xlib::Xlib::open() {
        unsafe {
            (xlib.XUngrabPointer)(display as *mut x11_dl::xlib::Display, 0);
            (xlib.XFlush)(display as *mut x11_dl::xlib::Display);
        }
    }
}

struct Atoms {
    aware: Atom,
    enter: Atom,
    position: Atom,
    status: Atom,
    leave: Atom,
    drop: Atom,
    finished: Atom,
    selection: Atom,
    action_copy: Atom,
    uri_list: Atom,
    targets: Atom,
}

fn atom(connection: &RustConnection, name: &str) -> Result<Atom, Failure> {
    Ok(connection.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

fn intern_all(connection: &RustConnection) -> Result<Atoms, Failure> {
    Ok(Atoms {
        aware: atom(connection, "XdndAware")?,
        enter: atom(connection, "XdndEnter")?,
        position: atom(connection, "XdndPosition")?,
        status: atom(connection, "XdndStatus")?,
        leave: atom(connection, "XdndLeave")?,
        drop: atom(connection, "XdndDrop")?,
        finished: atom(connection, "XdndFinished")?,
        selection: atom(connection, "XdndSelection")?,
        action_copy: atom(connection, "XdndActionCopy")?,
        uri_list: atom(connection, "text/uri-list")?,
        targets: atom(connection, "TARGETS")?,
    })
}

fn send_message(connection: &RustConnection, to: Window, kind: Atom, data: [u32; 5]) -> Result<(), Failure> {
    let event = ClientMessageEvent::new(32, to, kind, data);
    connection.send_event(false, to, EventMask::NO_EVENT, event)?;
    connection.flush()?;
    Ok(())
}

fn aware_version(connection: &RustConnection, window: Window, aware: Atom) -> Result<Option<u32>, Failure> {
    let reply = connection.get_property(false, window, aware, AtomEnum::ATOM, 0, 1)?.reply()?;
    Ok(reply.value32().and_then(|mut values| values.next()))
}

fn target_under_pointer(
    connection: &RustConnection, root: Window, aware: Atom, source: Window,
) -> Result<Option<(Window, u32)>, Failure> {
    let mut deepest = root;
    loop {
        let reply = connection.query_pointer(deepest)?.reply()?;
        if reply.child == 0 || reply.child == source {
            break;
        }
        deepest = reply.child;
    }
    let mut candidate = deepest;
    while candidate != root && candidate != 0 {
        if let Some(version) = aware_version(connection, candidate, aware)? {
            return Ok(Some((candidate, version.min(XDND_VERSION))));
        }
        candidate = connection.query_tree(candidate)?.reply()?.parent;
    }
    Ok(None)
}

fn answer_selection_request(
    connection: &RustConnection, atoms: &Atoms, request: &SelectionRequestEvent, body: &[u8],
) -> Result<(), Failure> {
    let property = if request.property == 0 { request.target } else { request.property };
    let mut answered = property;
    if request.target == atoms.uri_list {
        connection.change_property8(PropMode::REPLACE, request.requestor, property, atoms.uri_list, body)?;
    } else if request.target == atoms.targets {
        connection.change_property32(
            PropMode::REPLACE,
            request.requestor,
            property,
            AtomEnum::ATOM,
            &[atoms.targets, atoms.uri_list],
        )?;
    } else {
        answered = 0;
    }
    let notify = SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: request.time,
        requestor: request.requestor,
        selection: request.selection,
        target: request.target,
        property: answered,
    };
    connection.send_event(false, request.requestor, EventMask::NO_EVENT, notify)?;
    connection.flush()?;
    Ok(())
}

fn packed_position(x: i16, y: i16) -> u32 {
    ((x as u32 & 0xffff) << 16) | (y as u32 & 0xffff)
}

fn run_drag(paths: &[PathBuf], events: &EventLog, cancelled: &AtomicBool) -> Result<NativeEvent, Failure> {
    let (connection, screen_number) = RustConnection::connect(None)?;
    let root = connection.setup().roots[screen_number].root;
    let atoms = intern_all(&connection)?;
    let body = uri::list(paths).into_bytes();

    let source = connection.generate_id()?;
    connection.create_window(
        0,
        source,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().override_redirect(1),
    )?;
    connection.set_selection_owner(source, atoms.selection, CURRENT_TIME)?;
    connection.flush()?;

    let grab = connection
        .grab_pointer(
            false,
            root,
            EventMask::POINTER_MOTION | EventMask::BUTTON_RELEASE,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
            0u32,
            0u32,
            CURRENT_TIME,
        )?
        .reply()?;
    if grab.status != GrabStatus::SUCCESS {
        connection.destroy_window(source)?;
        connection.flush()?;
        return Err(format!("the pointer grab was refused: {:?}", grab.status).into());
    }
    events.push(NativeEvent::Started);
    let _ = connection.grab_keyboard(false, root, CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC)?.reply()?;

    let outcome = drag_loop(&connection, root, source, &atoms, &body, cancelled);

    connection.ungrab_pointer(CURRENT_TIME)?;
    connection.ungrab_keyboard(CURRENT_TIME)?;
    connection.destroy_window(source)?;
    connection.flush()?;
    outcome
}

fn drag_loop(
    connection: &RustConnection, root: Window, source: Window, atoms: &Atoms, body: &[u8], cancelled: &AtomicBool,
) -> Result<NativeEvent, Failure> {
    let mut target: Option<(Window, u32)> = None;
    let mut accepted = false;

    loop {
        if cancelled.load(Ordering::SeqCst) {
            if let Some((window, _)) = target {
                send_message(connection, window, atoms.leave, [source, 0, 0, 0, 0])?;
            }
            return Ok(NativeEvent::Cancelled);
        }
        let Some(event) = connection.poll_for_event()? else {
            std::thread::sleep(IDLE_WAIT);
            continue;
        };
        match event {
            Event::MotionNotify(motion) => {
                let found = target_under_pointer(connection, root, atoms.aware, source)?;
                if found.map(|(window, _)| window) != target.map(|(window, _)| window) {
                    if let Some((old, _)) = target {
                        send_message(connection, old, atoms.leave, [source, 0, 0, 0, 0])?;
                    }
                    accepted = false;
                    target = found;
                    if let Some((window, version)) = target {
                        send_message(connection, window, atoms.enter, [source, version << 24, atoms.uri_list, 0, 0])?;
                    }
                }
                if let Some((window, _)) = target {
                    let packed = packed_position(motion.root_x, motion.root_y);
                    send_message(
                        connection,
                        window,
                        atoms.position,
                        [source, 0, packed, motion.time, atoms.action_copy],
                    )?;
                }
            }
            Event::ClientMessage(message) if message.type_ == atoms.status => {
                accepted = message.data.as_data32()[1] & 1 == 1;
            }
            Event::SelectionRequest(request) => answer_selection_request(connection, atoms, &request, body)?,
            Event::KeyPress(key) if key.detail == ESCAPE_KEYCODE => {
                if let Some((window, _)) = target {
                    send_message(connection, window, atoms.leave, [source, 0, 0, 0, 0])?;
                }
                return Ok(NativeEvent::Cancelled);
            }
            Event::ButtonRelease(release) => {
                let Some((window, _)) = target else {
                    return Ok(NativeEvent::Cancelled);
                };
                if !accepted {
                    send_message(connection, window, atoms.leave, [source, 0, 0, 0, 0])?;
                    return Ok(NativeEvent::Cancelled);
                }
                send_message(connection, window, atoms.drop, [source, 0, release.time, 0, 0])?;
                return wait_for_finish(connection, atoms, body);
            }
            _ => {}
        }
    }
}

fn wait_for_finish(connection: &RustConnection, atoms: &Atoms, body: &[u8]) -> Result<NativeEvent, Failure> {
    let deadline = Instant::now() + FINISH_TIMEOUT;
    while Instant::now() < deadline {
        match connection.poll_for_event()? {
            Some(Event::SelectionRequest(request)) => answer_selection_request(connection, atoms, &request, body)?,
            Some(Event::ClientMessage(message)) if message.type_ == atoms.finished => return Ok(NativeEvent::Finished),
            Some(_) => {}
            None => std::thread::sleep(IDLE_WAIT),
        }
    }
    Ok(NativeEvent::Finished)
}

#[cfg(test)]
mod tests;
