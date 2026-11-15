use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use iced::Point;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use tokio::sync::mpsc::UnboundedSender;
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_pointer::{self, ButtonState, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop, event_created_child};

use super::{DesktopEvent, EventLog, Native, NativeError, NativeEvent, uri};

const URI_LIST: &str = "text/uri-list";
const LEFT_BUTTON: u32 = 0x110;

#[derive(Default)]
struct Shared {
    serial: AtomicU32,
    left_down: AtomicBool,
    generation: AtomicU64,
    paths: Mutex<Vec<PathBuf>>,
    events: EventLog,
}

struct AppData {
    sender: UnboundedSender<DesktopEvent>,
    shared: Arc<Shared>,
    surface: ObjectId,
    offers: HashMap<ObjectId, Vec<String>>,
    current_offer: Option<WlDataOffer>,
    last_position: Point,
}

impl AppData {
    fn send(&self, event: DesktopEvent) {
        let _ = self.sender.send(event);
    }
}

pub struct WaylandNative {
    connection: Connection,
    qh: QueueHandle<AppData>,
    data_device: WlDataDevice,
    manager: WlDataDeviceManager,
    surface: WlSurface,
    shared: Arc<Shared>,
    source: Option<WlDataSource>,
}

impl fmt::Debug for WaylandNative {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WaylandNative")
    }
}

impl WaylandNative {
    pub fn attach(window: &dyn iced::window::Window, sender: UnboundedSender<DesktopEvent>) -> Option<WaylandNative> {
        let RawWindowHandle::Wayland(surface_handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let RawDisplayHandle::Wayland(display_handle) = window.display_handle().ok()?.as_raw() else {
            return None;
        };
        let backend = unsafe { Backend::from_foreign_display(display_handle.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let (globals, mut queue) = registry_queue_init::<AppData>(&connection).ok()?;
        let qh = queue.handle();
        let seat: WlSeat = globals.bind(&qh, 1..=7, ()).ok()?;
        let manager: WlDataDeviceManager = globals.bind(&qh, 1..=3, ()).ok()?;
        let _pointer: WlPointer = seat.get_pointer(&qh, ());
        let data_device = manager.get_data_device(&seat, &qh, ());
        let surface_id =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), surface_handle.surface.as_ptr().cast()).ok()? };
        let surface = WlSurface::from_id(&connection, surface_id.clone()).ok()?;
        let shared = Arc::new(Shared::default());
        let mut app = AppData {
            sender,
            shared: shared.clone(),
            surface: surface_id,
            offers: HashMap::new(),
            current_offer: None,
            last_position: Point::ORIGIN,
        };
        std::thread::spawn(move || while queue.blocking_dispatch(&mut app).is_ok() {});
        Some(WaylandNative { connection, qh, data_device, manager, surface, shared, source: None })
    }
}

impl Native for WaylandNative {
    fn start_drag(&mut self, paths: Vec<PathBuf>) -> Result<(), NativeError> {
        if !self.shared.left_down.load(Ordering::SeqCst) {
            return Err(NativeError("the left button is no longer down, so a drag cannot start".to_string()));
        }
        let serial = self.shared.serial.load(Ordering::SeqCst);
        *self.shared.paths.lock().unwrap() = paths;
        if let Some(previous) = self.source.take() {
            previous.destroy();
        }
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let source = self.manager.create_data_source(&self.qh, generation);
        source.offer(URI_LIST.to_string());
        source.set_actions(DndAction::Copy);
        self.data_device.start_drag(Some(&source), &self.surface, None, serial);
        self.connection.flush().map_err(|error| NativeError(error.to_string()))?;
        self.source = Some(source);
        self.shared.events.push(NativeEvent::Started);
        Ok(())
    }

    fn cancel(&mut self) {
        if let Some(source) = self.source.take() {
            source.destroy();
            self.shared.generation.fetch_add(1, Ordering::SeqCst);
            let _ = self.connection.flush();
            self.shared.events.push(NativeEvent::Cancelled);
        }
    }

    fn poll(&mut self) -> Vec<NativeEvent> {
        self.shared.events.take()
    }
}

delegate_noop!(AppData: ignore WlSeat);
delegate_noop!(AppData: ignore WlDataDeviceManager);

impl Dispatch<WlRegistry, GlobalListContents> for AppData {
    fn event(
        _: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event, _: &GlobalListContents, _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlPointer, ()> for AppData {
    fn event(state: &mut Self, _: &WlPointer, event: wl_pointer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_pointer::Event::Button { serial, button, state: WEnum::Value(button_state), .. } = event
            && button == LEFT_BUTTON
        {
            let pressed = button_state == ButtonState::Pressed;
            state.shared.left_down.store(pressed, Ordering::SeqCst);
            if pressed {
                state.shared.serial.store(serial, Ordering::SeqCst);
            }
        }
    }
}

impl Dispatch<WlDataSource, u64> for AppData {
    fn event(
        state: &mut Self, _: &WlDataSource, event: wl_data_source::Event, generation: &u64, _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type, fd } if mime_type == URI_LIST => {
                let paths = state.shared.paths.lock().unwrap().clone();
                let _ = std::fs::File::from(fd).write_all(uri::list(&paths).as_bytes());
            }
            wl_data_source::Event::Cancelled => end_drag(&state.shared, *generation, NativeEvent::Cancelled),
            wl_data_source::Event::DndFinished => end_drag(&state.shared, *generation, NativeEvent::Finished),
            _ => {}
        }
    }
}

fn end_drag(shared: &Shared, generation: u64, event: NativeEvent) {
    if shared.generation.compare_exchange(generation, generation + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        shared.events.push(event);
    }
}

impl Dispatch<WlDataOffer, ()> for AppData {
    fn event(
        state: &mut Self, offer: &WlDataOffer, event: wl_data_offer::Event, _: &(), _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            state.offers.entry(offer.id()).or_default().push(mime_type);
        }
    }
}

impl Dispatch<WlDataDevice, ()> for AppData {
    event_created_child!(AppData, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);

    fn event(
        state: &mut Self, _: &WlDataDevice, event: wl_data_device::Event, _: &(), _: &Connection, _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter { serial, surface, x, y, id } => {
                if surface.id() != state.surface {
                    return;
                }
                let Some(offer) = id else { return };
                let wants = state.offers.get(&offer.id()).is_some_and(|mimes| mimes.iter().any(|m| m == URI_LIST));
                if wants {
                    offer.accept(serial, Some(URI_LIST.to_string()));
                    offer.set_actions(DndAction::Copy, DndAction::Copy);
                }
                state.last_position = Point::new(x as f32, y as f32);
                state.current_offer = Some(offer);
                state.send(DesktopEvent::Hover(Vec::new()));
                state.send(DesktopEvent::Moved(state.last_position));
            }
            wl_data_device::Event::Motion { x, y, .. } => {
                state.last_position = Point::new(x as f32, y as f32);
                if state.current_offer.is_some() {
                    state.send(DesktopEvent::Moved(state.last_position));
                }
            }
            wl_data_device::Event::Leave => {
                if state.current_offer.take().is_some() {
                    state.send(DesktopEvent::Left);
                }
            }
            wl_data_device::Event::Drop => {
                let Some(offer) = state.current_offer.take() else { return };
                let Ok((reader, writer)) = rustix::pipe::pipe() else { return };
                offer.receive(URI_LIST.to_string(), writer.as_fd());
                drop(writer);
                let sender = state.sender.clone();
                let at = Some(state.last_position);
                std::thread::spawn(move || {
                    let mut text = String::new();
                    let _ = std::fs::File::from(reader).read_to_string(&mut text);
                    offer.finish();
                    let _ = sender.send(DesktopEvent::Dropped { paths: uri::parse(&text), at });
                });
            }
            _ => {}
        }
    }
}
