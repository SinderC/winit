//! Precision touchpad scrolling through DirectManipulation.
//!
//! Windows sends a precision touchpad's scrolling as `WM_MOUSEWHEEL`, which says neither when the
//! fingers land and lift nor which events are the system's momentum. A DirectManipulation
//! viewport on the window recognizes the gesture instead, as Chromium, Firefox and Zed do: it
//! takes the touchpad's contact on `DM_POINTERHITTEST`, and the transform of its content follows
//! the fingers, then the inertia once they lift. The viewport draws nothing. The changes in its
//! translation become [`WindowEvent::MouseWheel`]s in pixels with phases, from
//! [`ScrollSource::Finger`] and then [`ScrollSource::Momentum`], and the changes in its scale
//! become [`WindowEvent::PinchGesture`]s, as on macOS.
//!
//! The viewport is updated by hand. While a gesture runs, a thread waits for each frame of the
//! desktop compositor and posts a message to the window, which then calls
//! [`DirectManipulation::on_frame`].

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::{fmt, mem};

use dpi::PhysicalPosition;
use windows_sys::Win32::Foundation::{E_NOINTERFACE, E_POINTER, HWND, RECT, S_OK, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows_sys::Win32::UI::Input::Pointer::GetPointerType;
use windows_sys::Win32::UI::WindowsAndMessaging::{POINTER_INPUT_TYPE, PT_TOUCHPAD, PostMessageW};
use windows_sys::core::{GUID, HRESULT};
use winit_core::event::{MouseScrollDelta, ScrollSource, TouchPhase, WindowEvent};

use crate::definitions::*;
use crate::util::loword;

/// The viewport's size, in physical pixels. It only recognizes gestures, so any size does: its
/// content's translation is read as changes, and put back to zero after each gesture.
const VIEWPORT_SIZE: i32 = 1000;

/// What the viewport follows: panning on either axis, locked to one axis when the pan starts
/// along it, with inertia, and pinching. Once the viewport has the touchpad's contact, Windows
/// no longer makes a pinch into Ctrl+wheel, so it must follow pinches too. They have no inertia,
/// as on macOS.
const CONFIGURATION: DIRECTMANIPULATION_CONFIGURATION = DIRECTMANIPULATION_CONFIGURATION_INTERACTION
    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X
    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y
    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_INERTIA
    | DIRECTMANIPULATION_CONFIGURATION_RAILS_X
    | DIRECTMANIPULATION_CONFIGURATION_RAILS_Y
    | DIRECTMANIPULATION_CONFIGURATION_SCALING;

/// Updated by hand, once per frame, and without rounding the translation to whole pixels.
const OPTIONS: DIRECTMANIPULATION_VIEWPORT_OPTIONS =
    DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE
        | DIRECTMANIPULATION_VIEWPORT_OPTIONS_DISABLEPIXELSNAPPING;

/// Frames to go on updating after a contact while no gesture runs yet, for DirectManipulation to
/// recognize a pan: half a second at 60 Hz. A contact that never pans, such as a tap, stops the
/// updates after this.
const CONTACT_GRACE_FRAMES: u32 = 30;

/// Frames to go on updating after the content is put back, for the viewport to take it.
const RESET_FRAMES: u32 = 2;

/// How long the frame thread waits for a frame when the compositor can't say: about one frame
/// at 60 Hz.
const FALLBACK_FRAME: Duration = Duration::from_millis(16);

/// A change in the content's translation smaller than this, in pixels, isn't sent.
const EPSILON: f32 = 0.01;

/// A content scaled by more than this, as a share, is being pinched. Panning leaves the scale at
/// exactly one, give or take float rounding.
const SCALE_EPSILON: f32 = 1e-4;

/// A failed DirectManipulation call.
#[derive(Debug)]
pub(crate) struct Error {
    call: &'static str,
    hr: HRESULT,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DirectManipulation's `{}` failed: 0x{:08x}", self.call, self.hr)
    }
}

impl std::error::Error for Error {}

fn check(call: &'static str, hr: HRESULT) -> Result<(), Error> {
    if hr < 0 { Err(Error { call, hr }) } else { Ok(()) }
}

/// A COM interface, with the type of its vtable.
trait Interface {
    type Vtbl;
}

impl Interface for IDirectManipulationManager {
    type Vtbl = IDirectManipulationManagerVtbl;
}

impl Interface for IDirectManipulationUpdateManager {
    type Vtbl = IDirectManipulationUpdateManagerVtbl;
}

impl Interface for IDirectManipulationViewport {
    type Vtbl = IDirectManipulationViewportVtbl;
}

/// One reference to a COM object, released when dropped.
struct Com<T: Interface>(NonNull<T>);

impl<T: Interface> Com<T> {
    /// The interface a call that returned `hr` put in its out parameter `object`.
    ///
    /// # Safety
    ///
    /// `object` is what the call wrote: on success, a `T` interface pointer holding one reference
    /// for the caller.
    unsafe fn take(call: &'static str, hr: HRESULT, object: *mut c_void) -> Result<Self, Error> {
        check(call, hr)?;
        NonNull::new(object.cast()).map(Self).ok_or(Error { call, hr: E_POINTER })
    }

    fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }

    fn vtbl(&self) -> &T::Vtbl {
        // SAFETY: `self` holds a reference to a live COM object, which begins with a pointer to
        // its vtable, alive as long as the object.
        unsafe { &**self.0.as_ptr().cast::<*const T::Vtbl>() }
    }
}

impl<T: Interface> Drop for Com<T> {
    fn drop(&mut self) {
        let unknown = self.0.as_ptr().cast::<IUnknown>();
        // SAFETY: every COM vtable begins with `IUnknown`'s, and `self` owns one reference.
        unsafe { ((**unknown.cast::<*const IUnknownVtbl>()).Release)(unknown) };
    }
}

/// A window's DirectManipulation viewport, turning touchpad gestures into scroll events.
pub(crate) struct DirectManipulation {
    window: HWND,
    manager: Com<IDirectManipulationManager>,
    update_manager: Com<IDirectManipulationUpdateManager>,
    viewport: Com<IDirectManipulationViewport>,
    /// One reference to the viewport's event handler.
    handler: NonNull<Handler>,
    /// The handler's registration with the viewport.
    cookie: u32,
    pump: FramePump,
    /// Frames left to update while no gesture runs.
    grace: Cell<u32>,
    destroyed: Cell<bool>,
}

impl DirectManipulation {
    /// Sets up a viewport for `window`, which posts itself `frame_message` while a gesture runs.
    ///
    /// COM must be initialized on this thread, apartment-threaded, and `window` must be one of
    /// its windows.
    pub(crate) fn new(window: HWND, frame_message: u32) -> Result<Self, Error> {
        let mut object = ptr::null_mut();
        // SAFETY: the GUIDs and the out pointer are valid for the call.
        let hr = unsafe {
            CoCreateInstance(
                &CLSID_DirectManipulationManager,
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IDirectManipulationManager,
                &mut object,
            )
        };
        // SAFETY: `object` is what `CoCreateInstance` wrote for this IID.
        let manager: Com<IDirectManipulationManager> =
            unsafe { Com::take("CoCreateInstance", hr, object) }?;

        let mut object = ptr::null_mut();
        // SAFETY: a live manager; the IID and the out pointer are valid for the call.
        let hr = unsafe {
            (manager.vtbl().GetUpdateManager)(
                manager.as_ptr(),
                &IID_IDirectManipulationUpdateManager,
                &mut object,
            )
        };
        // SAFETY: `object` is what `GetUpdateManager` wrote for this IID.
        let update_manager = unsafe { Com::take("GetUpdateManager", hr, object) }?;

        let mut object = ptr::null_mut();
        // SAFETY: a live manager, no frame info provider (the viewport is updated by hand), a
        // window of this thread, and an IID and out pointer valid for the call.
        let hr = unsafe {
            (manager.vtbl().CreateViewport)(
                manager.as_ptr(),
                ptr::null_mut(),
                window,
                &IID_IDirectManipulationViewport,
                &mut object,
            )
        };
        // SAFETY: `object` is what `CreateViewport` wrote for this IID.
        let viewport: Com<IDirectManipulationViewport> =
            unsafe { Com::take("CreateViewport", hr, object) }?;

        let vtbl = viewport.vtbl();
        let rect = RECT { left: 0, top: 0, right: VIEWPORT_SIZE, bottom: VIEWPORT_SIZE };
        // SAFETY: a live viewport, and values from `directmanipulation.h`.
        unsafe {
            check(
                "ActivateConfiguration",
                (vtbl.ActivateConfiguration)(viewport.as_ptr(), CONFIGURATION),
            )?;
            check("SetViewportOptions", (vtbl.SetViewportOptions)(viewport.as_ptr(), OPTIONS))?;
            check("SetViewportRect", (vtbl.SetViewportRect)(viewport.as_ptr(), &rect))?;
        }

        let handler = Handler::new();
        let mut cookie = 0;
        // SAFETY: a live viewport and handler; the viewport takes its own reference to the
        // handler, and calls it back on `window`'s thread.
        let hr = unsafe {
            (vtbl.AddEventHandler)(viewport.as_ptr(), window, handler.as_ptr().cast(), &mut cookie)
        };
        if let Err(err) = check("AddEventHandler", hr) {
            // SAFETY: the reference `Handler::new` made, now unused.
            unsafe { Handler::release(handler.as_ptr().cast()) };
            return Err(err);
        }

        // From here on, dropping `this` undoes the setup.
        let this = Self {
            window,
            manager,
            update_manager,
            viewport,
            handler,
            cookie,
            pump: FramePump::new(window, frame_message),
            grace: Cell::new(0),
            destroyed: Cell::new(false),
        };
        // SAFETY: a live manager and viewport, and a window of this thread.
        unsafe {
            check("Activate", (this.manager.vtbl().Activate)(this.manager.as_ptr(), window))?;
            check("Enable", (this.viewport.vtbl().Enable)(this.viewport.as_ptr()))?;
        }
        this.update();
        Ok(this)
    }

    /// A `DM_POINTERHITTEST`: hands a touchpad's contact to the viewport, which then follows it.
    pub(crate) fn on_pointer_hit_test(&self, wparam: WPARAM) {
        if self.destroyed.get() {
            return;
        }
        let pointer_id = u32::from(loword(wparam as u32));
        let mut kind: POINTER_INPUT_TYPE = 0;
        // SAFETY: the out pointer is valid for the call.
        if unsafe { GetPointerType(pointer_id, &mut kind) } == 0 || kind != PT_TOUCHPAD {
            return;
        }
        // A contact is the user's: it isn't the content being put back.
        self.with_gesture(|gesture| gesture.resetting = false);
        // SAFETY: a live viewport.
        let hr = unsafe { (self.viewport.vtbl().SetContact)(self.viewport.as_ptr(), pointer_id) };
        if let Err(err) = check("SetContact", hr) {
            tracing::warn!("{err}");
            return;
        }
        self.grace.set(CONTACT_GRACE_FRAMES);
        self.pump.start();
    }

    /// A frame while a gesture runs: updates the viewport, and returns the events it made.
    pub(crate) fn on_frame(&self) -> Vec<WindowEvent> {
        self.pump.frame_handled();
        if self.destroyed.get() {
            return Vec::new();
        }
        self.update();
        let (events, reset) = self
            .with_gesture(|gesture| (mem::take(&mut gesture.events), mem::take(&mut gesture.reset)))
            .unwrap_or_default();
        if reset {
            self.reset();
        }
        if !self.keeps_updating() {
            self.pump.stop();
        }
        events
    }

    /// Whether a gesture moves the viewport, so that a `WM_MOUSEWHEEL` would scroll twice.
    pub(crate) fn is_scrolling(&self) -> bool {
        matches!(self.status(), DIRECTMANIPULATION_RUNNING | DIRECTMANIPULATION_INERTIA)
    }

    /// Undoes the setup, before the window is destroyed. Later calls do nothing.
    pub(crate) fn destroy(&self) {
        if self.destroyed.replace(true) {
            return;
        }
        self.pump.quit();
        let vtbl = self.viewport.vtbl();
        let viewport = self.viewport.as_ptr();
        // SAFETY: a live viewport and manager, and the window they were set up for. Failures
        // leave nothing to undo.
        unsafe {
            (vtbl.RemoveEventHandler)(viewport, self.cookie);
            (vtbl.Stop)(viewport);
            (vtbl.Abandon)(viewport);
            (self.manager.vtbl().Deactivate)(self.manager.as_ptr(), self.window);
        }
    }

    fn update(&self) {
        // SAFETY: a live update manager; no frame info provider, as the viewport is updated by
        // hand.
        let hr = unsafe {
            (self.update_manager.vtbl().Update)(self.update_manager.as_ptr(), ptr::null_mut())
        };
        if let Err(err) = check("Update", hr) {
            tracing::warn!("{err}");
        }
    }

    /// Puts the content back where it began, for the next gesture.
    fn reset(&self) {
        self.with_gesture(|gesture| gesture.resetting = true);
        let size = VIEWPORT_SIZE as f32;
        // SAFETY: a live viewport.
        let hr = unsafe {
            (self.viewport.vtbl().ZoomToRect)(self.viewport.as_ptr(), 0.0, 0.0, size, size, 0)
        };
        if let Err(err) = check("ZoomToRect", hr) {
            tracing::warn!("{err}");
            self.with_gesture(|gesture| gesture.resetting = false);
        }
        self.grace.set(self.grace.get().max(RESET_FRAMES));
    }

    /// Whether to update the viewport on the next frame too.
    fn keeps_updating(&self) -> bool {
        if self.is_scrolling() {
            return true;
        }
        let grace = self.grace.get();
        self.grace.set(grace.saturating_sub(1));
        grace > 0
    }

    fn status(&self) -> DIRECTMANIPULATION_STATUS {
        let mut status = DIRECTMANIPULATION_READY;
        // SAFETY: a live viewport, and an out pointer valid for the call.
        let hr = unsafe { (self.viewport.vtbl().GetStatus)(self.viewport.as_ptr(), &mut status) };
        if hr < 0 { DIRECTMANIPULATION_READY } else { status }
    }

    /// Runs `f` on the gesture, unless a callback has it.
    fn with_gesture<R>(&self, f: impl FnOnce(&mut Gesture) -> R) -> Option<R> {
        // SAFETY: `self` holds a reference to the handler, so it is alive.
        let handler = unsafe { self.handler.as_ref() };
        handler.gesture.try_borrow_mut().ok().map(|mut gesture| f(&mut gesture))
    }
}

impl Drop for DirectManipulation {
    fn drop(&mut self) {
        self.destroy();
        // SAFETY: the reference `Handler::new` made for `self`.
        unsafe { Handler::release(self.handler.as_ptr().cast()) };
    }
}

/// What a gesture does with the content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Motion {
    /// No gesture runs.
    #[default]
    None,
    /// Scrolls it: the fingers, or their momentum.
    Scroll(ScrollSource),
    /// Zooms it. A pan may turn into a pinch, as two fingers seldom land and spread at once,
    /// but a pinch stays one until the fingers lift.
    Pinch,
    /// Neither, for the rest of the gesture: the inertia after a pinch.
    Ignored,
}

/// A gesture as the viewport reports it, and the events it makes.
struct Gesture {
    motion: Motion,
    /// Whether a `Started` event went out for `motion`.
    started: bool,
    /// The content's translation at the last change, in physical pixels.
    last: (f32, f32),
    /// The content's scale at the last change.
    last_scale: f32,
    /// The gesture ended with the content moved: it is put back after the update.
    reset: bool,
    /// The content is being put back: that moves it, but isn't a gesture.
    resetting: bool,
    /// The events made, sent once DirectManipulation's call has returned.
    events: Vec<WindowEvent>,
}

impl Default for Gesture {
    fn default() -> Self {
        Self {
            motion: Motion::None,
            started: false,
            last: (0.0, 0.0),
            last_scale: 1.0,
            reset: false,
            resetting: false,
            events: Vec::new(),
        }
    }
}

impl Gesture {
    fn status_changed(
        &mut self,
        current: DIRECTMANIPULATION_STATUS,
        previous: DIRECTMANIPULATION_STATUS,
    ) {
        if current == previous {
            return;
        }
        match current {
            DIRECTMANIPULATION_RUNNING if !self.resetting => {
                self.begin(Motion::Scroll(ScrollSource::Finger))
            },
            DIRECTMANIPULATION_INERTIA => {
                let next = if self.motion == Motion::Pinch {
                    Motion::Ignored
                } else {
                    Motion::Scroll(ScrollSource::Momentum)
                };
                self.begin(next);
            },
            DIRECTMANIPULATION_READY => {
                self.begin(Motion::None);
                // Putting the content back may leave float noise in its transform: that is
                // left be, as the deltas are changes, or each reset would ask for another.
                let was_resetting = mem::take(&mut self.resetting);
                if !was_resetting && (self.last != (0.0, 0.0) || self.last_scale != 1.0) {
                    self.reset = true;
                    // Where the content is put back to.
                    self.last = (0.0, 0.0);
                    self.last_scale = 1.0;
                }
            },
            _ => {},
        }
    }

    /// The content's transform changed: scaled by `scale`, moved to `(x, y)`.
    fn content_moved(&mut self, scale: f32, x: f32, y: f32) {
        let (dx, dy) = (x - self.last.0, y - self.last.1);
        let zoom = scale / self.last_scale - 1.0;
        self.last = (x, y);
        self.last_scale = scale;
        if self.resetting || scale <= 0.0 {
            return;
        }
        if matches!(self.motion, Motion::Scroll(_)) && (scale - 1.0).abs() > SCALE_EPSILON {
            self.begin(Motion::Pinch);
        }
        match self.motion {
            Motion::Scroll(source) => {
                if dx.abs() >= EPSILON || dy.abs() >= EPSILON {
                    let phase = self.next_phase();
                    self.push_scroll(source, phase, dx, dy);
                }
            },
            Motion::Pinch => {
                let phase = self.next_phase();
                self.push_pinch(phase, zoom);
            },
            Motion::None | Motion::Ignored => {},
        }
    }

    /// Ends what the gesture did, and goes on with `motion`.
    fn begin(&mut self, motion: Motion) {
        if self.started {
            match self.motion {
                Motion::Scroll(source) => self.push_scroll(source, TouchPhase::Ended, 0.0, 0.0),
                Motion::Pinch => self.push_pinch(TouchPhase::Ended, 0.0),
                Motion::None | Motion::Ignored => {},
            }
        }
        self.motion = motion;
        self.started = false;
    }

    fn next_phase(&mut self) -> TouchPhase {
        let phase = if self.started { TouchPhase::Moved } else { TouchPhase::Started };
        self.started = true;
        phase
    }

    fn push_scroll(&mut self, source: ScrollSource, phase: TouchPhase, dx: f32, dy: f32) {
        self.events.push(WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(dx.into(), dy.into())),
            phase,
            source,
        });
    }

    fn push_pinch(&mut self, phase: TouchPhase, delta: f32) {
        self.events.push(WindowEvent::PinchGesture {
            device_id: None,
            delta: delta.into(),
            phase,
        });
    }
}

/// The viewport's event handler: a COM object, called back on the window's thread.
#[repr(C)]
struct Handler {
    /// First, so that a pointer to the handler is a pointer to the interface.
    interface: IDirectManipulationViewportEventHandler,
    refcount: AtomicU32,
    gesture: RefCell<Gesture>,
}

static HANDLER_VTBL: IDirectManipulationViewportEventHandlerVtbl =
    IDirectManipulationViewportEventHandlerVtbl {
        parent: IUnknownVtbl {
            QueryInterface: Handler::query_interface,
            AddRef: Handler::add_ref,
            Release: Handler::release,
        },
        OnViewportStatusChanged: Handler::on_viewport_status_changed,
        OnViewportUpdated: Handler::on_viewport_updated,
        OnContentUpdated: Handler::on_content_updated,
    };

impl Handler {
    /// A handler with one reference, for the caller.
    fn new() -> NonNull<Handler> {
        NonNull::from(Box::leak(Box::new(Handler {
            interface: IDirectManipulationViewportEventHandler { lpVtbl: &HANDLER_VTBL },
            refcount: AtomicU32::new(1),
            gesture: RefCell::default(),
        })))
    }

    /// # Safety
    ///
    /// `this` is a live handler.
    unsafe fn from_interface<'a, T>(this: *mut T) -> &'a Handler {
        unsafe { &*this.cast::<Handler>() }
    }

    unsafe extern "system" fn query_interface(
        this: *mut IUnknown,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT {
        if riid.is_null() || object.is_null() {
            return E_POINTER;
        }
        // SAFETY: COM passes a valid IID and out pointer, checked for null above.
        unsafe {
            let riid = &*riid;
            if same_guid(riid, &IID_IUnknown)
                || same_guid(riid, &IID_IDirectManipulationViewportEventHandler)
            {
                Self::add_ref(this);
                *object = this.cast();
                S_OK
            } else {
                *object = ptr::null_mut();
                E_NOINTERFACE
            }
        }
    }

    unsafe extern "system" fn add_ref(this: *mut IUnknown) -> u32 {
        // SAFETY: COM calls this through a live handler.
        let handler = unsafe { Self::from_interface(this) };
        handler.refcount.fetch_add(1, Ordering::Relaxed) + 1
    }

    unsafe extern "system" fn release(this: *mut IUnknown) -> u32 {
        // SAFETY: COM calls this through a live handler.
        let handler = unsafe { Self::from_interface(this) };
        let count = handler.refcount.fetch_sub(1, Ordering::AcqRel) - 1;
        if count == 0 {
            // SAFETY: the last reference is gone; the handler came from `Box::leak` in `new`.
            drop(unsafe { Box::from_raw(this.cast::<Handler>()) });
        }
        count
    }

    unsafe extern "system" fn on_viewport_status_changed(
        this: *mut IDirectManipulationViewportEventHandler,
        _viewport: *mut IDirectManipulationViewport,
        current: DIRECTMANIPULATION_STATUS,
        previous: DIRECTMANIPULATION_STATUS,
    ) -> HRESULT {
        // SAFETY: DirectManipulation calls back through the handler it was given, alive while it
        // holds a reference.
        let handler = unsafe { Self::from_interface(this) };
        // No panic may cross this boundary: a call made from inside `with_gesture` leaves the
        // gesture be.
        if let Ok(mut gesture) = handler.gesture.try_borrow_mut() {
            gesture.status_changed(current, previous);
        }
        S_OK
    }

    unsafe extern "system" fn on_viewport_updated(
        _this: *mut IDirectManipulationViewportEventHandler,
        _viewport: *mut IDirectManipulationViewport,
    ) -> HRESULT {
        S_OK
    }

    unsafe extern "system" fn on_content_updated(
        this: *mut IDirectManipulationViewportEventHandler,
        _viewport: *mut IDirectManipulationViewport,
        content: *mut IDirectManipulationContent,
    ) -> HRESULT {
        if content.is_null() {
            return E_POINTER;
        }
        // The content's transform: scale x, 0, 0, scale y, translation x, translation y.
        let mut matrix = [0.0f32; 6];
        // SAFETY: `content` is a live content object for the call, and `matrix` holds the six
        // values asked for.
        let hr =
            unsafe { ((*(*content).lpVtbl).GetContentTransform)(content, matrix.as_mut_ptr(), 6) };
        if hr < 0 {
            return hr;
        }
        // SAFETY: as in `on_viewport_status_changed`.
        let handler = unsafe { Self::from_interface(this) };
        if let Ok(mut gesture) = handler.gesture.try_borrow_mut() {
            gesture.content_moved(matrix[0], matrix[4], matrix[5]);
        }
        S_OK
    }
}

fn same_guid(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

/// Posts a message to the window once per frame of the desktop compositor, while started.
struct FramePump {
    window: HWND,
    message: u32,
    shared: Arc<PumpState>,
    thread: RefCell<Option<JoinHandle<()>>>,
}

#[derive(Default)]
struct PumpState {
    running: AtomicBool,
    /// A message is posted and not yet handled: the window gets at most one at a time.
    posted: AtomicBool,
    quit: AtomicBool,
}

impl FramePump {
    fn new(window: HWND, message: u32) -> Self {
        Self { window, message, shared: Arc::default(), thread: RefCell::new(None) }
    }

    fn start(&self) {
        self.shared.running.store(true, Ordering::Release);
        let mut thread = self.thread.borrow_mut();
        if let Some(thread) = &*thread {
            thread.thread().unpark();
            return;
        }
        let shared = Arc::clone(&self.shared);
        // A window handle is a number to post to, valid on any thread.
        let window = self.window as usize;
        let message = self.message;
        match thread::Builder::new()
            .name("winit DirectManipulation frames".into())
            .spawn(move || pump(&shared, window, message))
        {
            Ok(handle) => *thread = Some(handle),
            Err(err) => {
                tracing::warn!("couldn't start the DirectManipulation frame thread: {err}");
                self.shared.running.store(false, Ordering::Release);
            },
        }
    }

    fn stop(&self) {
        self.shared.running.store(false, Ordering::Release);
    }

    fn frame_handled(&self) {
        self.shared.posted.store(false, Ordering::Release);
    }

    /// Ends the thread, before the window goes.
    fn quit(&self) {
        self.shared.quit.store(true, Ordering::Release);
        self.shared.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.borrow_mut().take() {
            thread.thread().unpark();
            // It waits for at most one frame.
            let _ = thread.join();
        }
    }
}

fn pump(state: &PumpState, window: usize, message: u32) {
    while !state.quit.load(Ordering::Acquire) {
        if !state.running.load(Ordering::Acquire) {
            // `start` and `quit` unpark: a wake-up that comes first makes this return at once.
            thread::park();
            continue;
        }
        // SAFETY: no arguments; it returns once the compositor has shown its next frame.
        if unsafe { DwmFlush() } < 0 {
            thread::sleep(FALLBACK_FRAME);
        }
        if state.running.load(Ordering::Acquire) && !state.posted.swap(true, Ordering::AcqRel) {
            // SAFETY: `quit` ends this thread before the window is destroyed, so `window` is
            // still the window; a failed post only means no frame.
            if unsafe { PostMessageW(window as HWND, message, 0, 0) } == 0 {
                state.posted.store(false, Ordering::Release);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each event as `(kind, phase, source)`, kind being "scroll" or "pinch".
    fn kinds(gesture: &mut Gesture) -> Vec<(&'static str, TouchPhase, Option<ScrollSource>)> {
        mem::take(&mut gesture.events)
            .into_iter()
            .map(|event| match event {
                WindowEvent::MouseWheel { phase, source, .. } => ("scroll", phase, Some(source)),
                WindowEvent::PinchGesture { phase, .. } => ("pinch", phase, None),
                other => panic!("unexpected event: {other:?}"),
            })
            .collect()
    }

    const BUILDING: DIRECTMANIPULATION_STATUS = 0;
    const FINGER: Option<ScrollSource> = Some(ScrollSource::Finger);
    const MOMENTUM: Option<ScrollSource> = Some(ScrollSource::Momentum);

    #[test]
    fn a_scroll_goes_from_the_fingers_to_their_momentum() {
        let mut gesture = Gesture::default();
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_READY);
        gesture.content_moved(1.0, 0.0, -10.0);
        gesture.content_moved(1.0, 0.0, -25.0);
        gesture.status_changed(DIRECTMANIPULATION_INERTIA, DIRECTMANIPULATION_RUNNING);
        gesture.content_moved(1.0, 0.0, -30.0);
        gesture.status_changed(DIRECTMANIPULATION_READY, DIRECTMANIPULATION_INERTIA);
        assert_eq!(kinds(&mut gesture), [
            ("scroll", TouchPhase::Started, FINGER),
            ("scroll", TouchPhase::Moved, FINGER),
            ("scroll", TouchPhase::Ended, FINGER),
            ("scroll", TouchPhase::Started, MOMENTUM),
            ("scroll", TouchPhase::Ended, MOMENTUM),
        ]);
        assert!(gesture.reset);
    }

    #[test]
    fn deltas_are_the_changes_in_the_translation() {
        let mut gesture = Gesture::default();
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_READY);
        gesture.content_moved(1.0, 3.0, -10.0);
        gesture.content_moved(1.0, 3.0, -25.0);
        let deltas: Vec<_> = gesture
            .events
            .iter()
            .map(|event| match event {
                WindowEvent::MouseWheel { delta: MouseScrollDelta::PixelDelta(p), .. } => {
                    (p.x, p.y)
                },
                other => panic!("unexpected event: {other:?}"),
            })
            .collect();
        assert_eq!(deltas, [(3.0, -10.0), (0.0, -15.0)]);
    }

    #[test]
    fn fingers_back_during_momentum_end_it() {
        let mut gesture = Gesture::default();
        gesture.status_changed(DIRECTMANIPULATION_INERTIA, DIRECTMANIPULATION_RUNNING);
        gesture.content_moved(1.0, 0.0, 20.0);
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_INERTIA);
        gesture.content_moved(1.0, 0.0, 25.0);
        assert_eq!(kinds(&mut gesture), [
            ("scroll", TouchPhase::Started, MOMENTUM),
            ("scroll", TouchPhase::Ended, MOMENTUM),
            ("scroll", TouchPhase::Started, FINGER),
        ]);
    }

    #[test]
    fn a_pan_that_spreads_becomes_a_pinch_and_its_inertia_is_ignored() {
        let mut gesture = Gesture::default();
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_READY);
        gesture.content_moved(1.0, 0.0, 2.0);
        gesture.content_moved(1.1, 0.0, 4.0);
        gesture.content_moved(1.21, 0.0, 6.0);
        gesture.content_moved(1.21, 0.0, 9.0);
        gesture.status_changed(DIRECTMANIPULATION_INERTIA, DIRECTMANIPULATION_RUNNING);
        gesture.content_moved(1.21, 0.0, 12.0);
        gesture.status_changed(DIRECTMANIPULATION_READY, DIRECTMANIPULATION_INERTIA);
        let zooms: Vec<_> = gesture
            .events
            .iter()
            .filter_map(|event| match event {
                WindowEvent::PinchGesture { delta, phase: TouchPhase::Started, .. }
                | WindowEvent::PinchGesture { delta, phase: TouchPhase::Moved, .. } => {
                    Some((delta * 100.0).round() / 100.0)
                },
                _ => None,
            })
            .collect();
        assert_eq!(zooms, [0.1, 0.1, 0.0]);
        assert_eq!(kinds(&mut gesture), [
            ("scroll", TouchPhase::Started, FINGER),
            ("scroll", TouchPhase::Ended, FINGER),
            ("pinch", TouchPhase::Started, None),
            ("pinch", TouchPhase::Moved, None),
            ("pinch", TouchPhase::Moved, None),
            ("pinch", TouchPhase::Ended, None),
        ]);
        assert!(gesture.reset);
        assert_eq!(gesture.last_scale, 1.0);
    }

    #[test]
    fn putting_the_content_back_sends_nothing_and_asks_for_no_more() {
        let mut gesture = Gesture::default();
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_READY);
        gesture.content_moved(1.0, 0.0, 40.0);
        gesture.status_changed(DIRECTMANIPULATION_READY, DIRECTMANIPULATION_RUNNING);
        assert!(mem::take(&mut gesture.reset));
        gesture.events.clear();

        // As `DirectManipulation::reset` does it, leaving a little float noise.
        gesture.resetting = true;
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_READY);
        gesture.content_moved(1.0, 0.0, 0.0001);
        gesture.status_changed(DIRECTMANIPULATION_READY, DIRECTMANIPULATION_RUNNING);
        assert!(gesture.events.is_empty());
        assert!(!gesture.resetting);
        assert!(!gesture.reset);
    }

    #[test]
    fn a_contact_that_never_moves_sends_nothing() {
        let mut gesture = Gesture::default();
        gesture.status_changed(DIRECTMANIPULATION_RUNNING, BUILDING);
        gesture.content_moved(1.0, 0.0, 0.0);
        gesture.status_changed(DIRECTMANIPULATION_READY, DIRECTMANIPULATION_RUNNING);
        assert!(gesture.events.is_empty());
        assert!(!gesture.reset);
    }
}
