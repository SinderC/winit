#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{HANDLE, HWND, POINT, POINTL, RECT};
use windows_sys::Win32::System::Com::{FORMATETC, STGMEDIUM};
use windows_sys::Win32::UI::Shell::SHDRAGIMAGE;
use windows_sys::Win32::UI::WindowsAndMessaging::MSG;
use windows_sys::core::{BOOL, GUID, HRESULT};

pub type IUnknown = *mut c_void;
pub type IAdviseSink = *mut c_void;
pub type IDataObject = *mut c_void;
pub type IEnumFORMATETC = *mut c_void;
pub type IEnumSTATDATA = *mut c_void;

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        This: *mut IUnknown,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(This: *mut IUnknown) -> u32,
    pub Release: unsafe extern "system" fn(This: *mut IUnknown) -> u32,
}

#[repr(C)]
pub struct IDataObjectVtbl {
    pub parent: IUnknownVtbl,
    pub GetData: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetcIn: *const FORMATETC,
        pmedium: *mut STGMEDIUM,
    ) -> HRESULT,
    pub GetDataHere: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetc: *const FORMATETC,
        pmedium: *mut STGMEDIUM,
    ) -> HRESULT,
    pub QueryGetData:
        unsafe extern "system" fn(This: *mut IDataObject, pformatetc: *const FORMATETC) -> HRESULT,
    pub GetCanonicalFormatEtc: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetcIn: *const FORMATETC,
        pformatetcOut: *mut FORMATETC,
    ) -> HRESULT,
    pub SetData: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetc: *const FORMATETC,
        pmedium: *const STGMEDIUM,
        fRelease: BOOL,
    ) -> HRESULT,
    pub EnumFormatEtc: unsafe extern "system" fn(
        This: *mut IDataObject,
        dwDirection: u32,
        ppenumFormatEtc: *mut *mut IEnumFORMATETC,
    ) -> HRESULT,
    pub DAdvise: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetc: *const FORMATETC,
        advf: u32,
        pAdvSInk: *const IAdviseSink,
        pdwConnection: *mut u32,
    ) -> HRESULT,
    pub DUnadvise: unsafe extern "system" fn(This: *mut IDataObject, dwConnection: u32) -> HRESULT,
    pub EnumDAdvise: unsafe extern "system" fn(
        This: *mut IDataObject,
        ppenumAdvise: *const *const IEnumSTATDATA,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IEnumFORMATETCVtbl {
    pub parent: IUnknownVtbl,
    pub Next: unsafe extern "system" fn(
        This: *mut IEnumFORMATETC,
        celt: u32,
        rgelt: *mut FORMATETC,
        pceltFetched: *mut u32,
    ) -> HRESULT,
    pub Skip: unsafe extern "system" fn(This: *mut IEnumFORMATETC, celt: u32) -> HRESULT,
    pub Reset: unsafe extern "system" fn(This: *mut IEnumFORMATETC) -> HRESULT,
    pub Clone: unsafe extern "system" fn(
        This: *mut IEnumFORMATETC,
        ppenum: *mut *mut IEnumFORMATETC,
    ) -> HRESULT,
}

pub type IDragSourceHelper = *mut c_void;

#[repr(C)]
pub struct IDragSourceHelperVtbl {
    pub parent: IUnknownVtbl,
    pub InitializeFromBitmap: unsafe extern "system" fn(
        This: *mut IDragSourceHelper,
        pshdi: *const SHDRAGIMAGE,
        pDataObject: *mut IDataObject,
    ) -> HRESULT,
    pub InitializeFromWindow: unsafe extern "system" fn(
        This: *mut IDragSourceHelper,
        hwnd: HWND,
        ppt: *const POINT,
        pDataObject: *mut IDataObject,
    ) -> HRESULT,
}

pub type IDropTargetHelper = *mut c_void;

#[repr(C)]
pub struct IDropTargetHelperVtbl {
    pub parent: IUnknownVtbl,
    pub DragEnter: unsafe extern "system" fn(
        This: *mut IDropTargetHelper,
        hwndTarget: HWND,
        pDataObject: *mut IDataObject,
        ppt: *const POINT,
        dwEffect: u32,
    ) -> HRESULT,
    pub DragLeave: unsafe extern "system" fn(This: *mut IDropTargetHelper) -> HRESULT,
    pub DragOver: unsafe extern "system" fn(
        This: *mut IDropTargetHelper,
        ppt: *const POINT,
        dwEffect: u32,
    ) -> HRESULT,
    pub Drop: unsafe extern "system" fn(
        This: *mut IDropTargetHelper,
        pDataObject: *mut IDataObject,
        ppt: *const POINT,
        dwEffect: u32,
    ) -> HRESULT,
    pub Show: unsafe extern "system" fn(This: *mut IDropTargetHelper, fShow: BOOL) -> HRESULT,
}

pub type IDropSource = *mut c_void;

#[repr(C)]
pub struct IDropSourceVtbl {
    pub parent: IUnknownVtbl,
    pub QueryContinueDrag: unsafe extern "system" fn(
        This: *mut IDropSource,
        fEscapePressed: BOOL,
        grfKeyState: u32,
    ) -> HRESULT,
    pub GiveFeedback: unsafe extern "system" fn(This: *mut IDropSource, dwEffect: u32) -> HRESULT,
}

#[repr(C)]
pub struct IDropTargetVtbl {
    pub parent: IUnknownVtbl,
    pub DragEnter: unsafe extern "system" fn(
        This: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT,
    pub DragOver: unsafe extern "system" fn(
        This: *mut IDropTarget,
        grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT,
    pub DragLeave: unsafe extern "system" fn(This: *mut IDropTarget) -> HRESULT,
    pub Drop: unsafe extern "system" fn(
        This: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDropTarget {
    pub lpVtbl: *const IDropTargetVtbl,
}

#[repr(C)]
pub struct ITaskbarListVtbl {
    pub parent: IUnknownVtbl,
    pub HrInit: unsafe extern "system" fn(This: *mut ITaskbarList) -> HRESULT,
    pub AddTab: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
    pub DeleteTab: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
    pub ActivateTab: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
    pub SetActiveAlt: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
}

#[repr(C)]
pub struct ITaskbarList {
    pub lpVtbl: *const ITaskbarListVtbl,
}

#[repr(C)]
pub struct ITaskbarList2Vtbl {
    pub parent: ITaskbarListVtbl,
    pub MarkFullscreenWindow: unsafe extern "system" fn(
        This: *mut ITaskbarList2,
        hwnd: HWND,
        fFullscreen: BOOL,
    ) -> HRESULT,
}

#[repr(C)]
pub struct ITaskbarList2 {
    pub lpVtbl: *const ITaskbarList2Vtbl,
}

// DirectManipulation, from `directmanipulation.h`. `windows-sys` leaves it out, as it does every
// COM interface. The vtables list every method in the header's order; winit calls only some.

pub type DIRECTMANIPULATION_STATUS = i32;
pub type DIRECTMANIPULATION_CONFIGURATION = i32;
pub type DIRECTMANIPULATION_VIEWPORT_OPTIONS = i32;

pub const DIRECTMANIPULATION_RUNNING: DIRECTMANIPULATION_STATUS = 3;
pub const DIRECTMANIPULATION_INERTIA: DIRECTMANIPULATION_STATUS = 4;
pub const DIRECTMANIPULATION_READY: DIRECTMANIPULATION_STATUS = 5;

pub const DIRECTMANIPULATION_CONFIGURATION_INTERACTION: DIRECTMANIPULATION_CONFIGURATION = 0x1;
pub const DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X: DIRECTMANIPULATION_CONFIGURATION = 0x2;
pub const DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y: DIRECTMANIPULATION_CONFIGURATION = 0x4;
pub const DIRECTMANIPULATION_CONFIGURATION_SCALING: DIRECTMANIPULATION_CONFIGURATION = 0x10;
pub const DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_INERTIA: DIRECTMANIPULATION_CONFIGURATION =
    0x20;
pub const DIRECTMANIPULATION_CONFIGURATION_RAILS_X: DIRECTMANIPULATION_CONFIGURATION = 0x100;
pub const DIRECTMANIPULATION_CONFIGURATION_RAILS_Y: DIRECTMANIPULATION_CONFIGURATION = 0x200;

pub const DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE: DIRECTMANIPULATION_VIEWPORT_OPTIONS =
    0x2;
pub const DIRECTMANIPULATION_VIEWPORT_OPTIONS_DISABLEPIXELSNAPPING:
    DIRECTMANIPULATION_VIEWPORT_OPTIONS = 0x10;

#[repr(C)]
pub struct IDirectManipulationManager {
    pub lpVtbl: *const IDirectManipulationManagerVtbl,
}

#[repr(C)]
pub struct IDirectManipulationManagerVtbl {
    pub parent: IUnknownVtbl,
    pub Activate:
        unsafe extern "system" fn(This: *mut IDirectManipulationManager, window: HWND) -> HRESULT,
    pub Deactivate:
        unsafe extern "system" fn(This: *mut IDirectManipulationManager, window: HWND) -> HRESULT,
    pub RegisterHitTestTarget: unsafe extern "system" fn(
        This: *mut IDirectManipulationManager,
        window: HWND,
        hitTestWindow: HWND,
        r#type: i32,
    ) -> HRESULT,
    pub ProcessInput: unsafe extern "system" fn(
        This: *mut IDirectManipulationManager,
        message: *const MSG,
        handled: *mut BOOL,
    ) -> HRESULT,
    pub GetUpdateManager: unsafe extern "system" fn(
        This: *mut IDirectManipulationManager,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateViewport: unsafe extern "system" fn(
        This: *mut IDirectManipulationManager,
        frameInfo: *mut c_void,
        window: HWND,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT,
    pub CreateContent: unsafe extern "system" fn(
        This: *mut IDirectManipulationManager,
        frameInfo: *mut c_void,
        clsid: *const GUID,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDirectManipulationUpdateManager {
    pub lpVtbl: *const IDirectManipulationUpdateManagerVtbl,
}

#[repr(C)]
pub struct IDirectManipulationUpdateManagerVtbl {
    pub parent: IUnknownVtbl,
    pub RegisterWaitHandleCallback: unsafe extern "system" fn(
        This: *mut IDirectManipulationUpdateManager,
        handle: HANDLE,
        eventHandler: *mut c_void,
        cookie: *mut u32,
    ) -> HRESULT,
    pub UnregisterWaitHandleCallback: unsafe extern "system" fn(
        This: *mut IDirectManipulationUpdateManager,
        cookie: u32,
    ) -> HRESULT,
    pub Update: unsafe extern "system" fn(
        This: *mut IDirectManipulationUpdateManager,
        frameInfo: *mut c_void,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDirectManipulationViewport {
    pub lpVtbl: *const IDirectManipulationViewportVtbl,
}

#[repr(C)]
pub struct IDirectManipulationViewportVtbl {
    pub parent: IUnknownVtbl,
    pub Enable: unsafe extern "system" fn(This: *mut IDirectManipulationViewport) -> HRESULT,
    pub Disable: unsafe extern "system" fn(This: *mut IDirectManipulationViewport) -> HRESULT,
    pub SetContact: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        pointerId: u32,
    ) -> HRESULT,
    pub ReleaseContact: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        pointerId: u32,
    ) -> HRESULT,
    pub ReleaseAllContacts:
        unsafe extern "system" fn(This: *mut IDirectManipulationViewport) -> HRESULT,
    pub GetStatus: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        status: *mut DIRECTMANIPULATION_STATUS,
    ) -> HRESULT,
    pub GetTag: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        riid: *const GUID,
        object: *mut *mut c_void,
        id: *mut u32,
    ) -> HRESULT,
    pub SetTag: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        object: *mut c_void,
        id: u32,
    ) -> HRESULT,
    pub GetViewportRect: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        viewport: *mut RECT,
    ) -> HRESULT,
    pub SetViewportRect: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        viewport: *const RECT,
    ) -> HRESULT,
    pub ZoomToRect: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        animate: BOOL,
    ) -> HRESULT,
    pub SetViewportTransform: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        matrix: *const f32,
        pointCount: u32,
    ) -> HRESULT,
    pub SyncDisplayTransform: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        matrix: *const f32,
        pointCount: u32,
    ) -> HRESULT,
    pub GetPrimaryContent: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT,
    pub AddContent: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        content: *mut IDirectManipulationContent,
    ) -> HRESULT,
    pub RemoveContent: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        content: *mut IDirectManipulationContent,
    ) -> HRESULT,
    pub SetViewportOptions: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        options: DIRECTMANIPULATION_VIEWPORT_OPTIONS,
    ) -> HRESULT,
    pub AddConfiguration: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        configuration: DIRECTMANIPULATION_CONFIGURATION,
    ) -> HRESULT,
    pub RemoveConfiguration: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        configuration: DIRECTMANIPULATION_CONFIGURATION,
    ) -> HRESULT,
    pub ActivateConfiguration: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        configuration: DIRECTMANIPULATION_CONFIGURATION,
    ) -> HRESULT,
    pub SetManualGesture: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        configuration: i32,
    ) -> HRESULT,
    pub SetChaining: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        enabledTypes: i32,
    ) -> HRESULT,
    pub AddEventHandler: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewport,
        window: HWND,
        eventHandler: *mut IDirectManipulationViewportEventHandler,
        cookie: *mut u32,
    ) -> HRESULT,
    pub RemoveEventHandler:
        unsafe extern "system" fn(This: *mut IDirectManipulationViewport, cookie: u32) -> HRESULT,
    pub SetInputMode:
        unsafe extern "system" fn(This: *mut IDirectManipulationViewport, mode: i32) -> HRESULT,
    pub SetUpdateMode:
        unsafe extern "system" fn(This: *mut IDirectManipulationViewport, mode: i32) -> HRESULT,
    pub Stop: unsafe extern "system" fn(This: *mut IDirectManipulationViewport) -> HRESULT,
    pub Abandon: unsafe extern "system" fn(This: *mut IDirectManipulationViewport) -> HRESULT,
}

#[repr(C)]
pub struct IDirectManipulationContent {
    pub lpVtbl: *const IDirectManipulationContentVtbl,
}

#[repr(C)]
pub struct IDirectManipulationContentVtbl {
    pub parent: IUnknownVtbl,
    pub GetContentRect: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        contentSize: *mut RECT,
    ) -> HRESULT,
    pub SetContentRect: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        contentSize: *const RECT,
    ) -> HRESULT,
    pub GetViewport: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        riid: *const GUID,
        object: *mut *mut c_void,
    ) -> HRESULT,
    pub GetTag: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        riid: *const GUID,
        object: *mut *mut c_void,
        id: *mut u32,
    ) -> HRESULT,
    pub SetTag: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        object: *mut c_void,
        id: u32,
    ) -> HRESULT,
    pub GetOutputTransform: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        matrix: *mut f32,
        pointCount: u32,
    ) -> HRESULT,
    pub GetContentTransform: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        matrix: *mut f32,
        pointCount: u32,
    ) -> HRESULT,
    pub SyncContentTransform: unsafe extern "system" fn(
        This: *mut IDirectManipulationContent,
        matrix: *const f32,
        pointCount: u32,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDirectManipulationViewportEventHandler {
    pub lpVtbl: *const IDirectManipulationViewportEventHandlerVtbl,
}

#[repr(C)]
pub struct IDirectManipulationViewportEventHandlerVtbl {
    pub parent: IUnknownVtbl,
    pub OnViewportStatusChanged: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewportEventHandler,
        viewport: *mut IDirectManipulationViewport,
        current: DIRECTMANIPULATION_STATUS,
        previous: DIRECTMANIPULATION_STATUS,
    ) -> HRESULT,
    pub OnViewportUpdated: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewportEventHandler,
        viewport: *mut IDirectManipulationViewport,
    ) -> HRESULT,
    pub OnContentUpdated: unsafe extern "system" fn(
        This: *mut IDirectManipulationViewportEventHandler,
        viewport: *mut IDirectManipulationViewport,
        content: *mut IDirectManipulationContent,
    ) -> HRESULT,
}

/// Defined in `unknwn.h`.
pub const IID_IUnknown: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);

/// Defined in `directmanipulation.h`.
pub const CLSID_DirectManipulationManager: GUID =
    GUID::from_u128(0x54e211b6_3650_4f75_8334_fa359598e1c5);

/// Defined in `directmanipulation.h`.
pub const IID_IDirectManipulationManager: GUID =
    GUID::from_u128(0xfbf5d3b4_70c7_4163_9322_5a6f660d6fbc);

/// Defined in `directmanipulation.h`.
pub const IID_IDirectManipulationUpdateManager: GUID =
    GUID::from_u128(0xb0ae62fd_be34_46e7_9caa_d361facbb9cc);

/// Defined in `directmanipulation.h`.
pub const IID_IDirectManipulationViewport: GUID =
    GUID::from_u128(0x28b85a3d_60a0_48bd_9ba1_5ce8d9ea3a6d);

/// Defined in `directmanipulation.h`.
pub const IID_IDirectManipulationViewportEventHandler: GUID =
    GUID::from_u128(0x952121da_d69f_45f9_b0f9_f23944321a6d);

/// Defined in `objidl.h`.
pub const IID_IDataObject: GUID = GUID::from_u128(0x0000010e_0000_0000_c000_000000000046);

/// Defined in `oleidl.h`.
pub const IID_IDropSource: GUID = GUID::from_u128(0x00000121_0000_0000_c000_000000000046);

/// Defined in `objidl.h`.
pub const IID_IEnumFORMATETC: GUID = GUID::from_u128(0x00000103_0000_0000_c000_000000000046);

/// Defined in `shobjidl_core.h`.
pub const IID_IDragSourceHelper: GUID = GUID::from_u128(0xde5bf786_477a_11d2_839d_00c04fd918d0);

/// Defined in `shobjidl_core.h`.
pub const IID_IDropTargetHelper: GUID = GUID::from_u128(0x4657278b_411b_11d2_839a_00c04fd918d0);

/// Defined in `shobjidl_core.h`.
pub const CLSID_TaskbarList: GUID = GUID {
    data1: 0x56fdf344,
    data2: 0xfd6d,
    data3: 0x11d0,
    data4: [0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90],
};

/// Defined in `shobjidl_core.h`.
pub const IID_ITaskbarList: GUID = GUID {
    data1: 0x56fdf342,
    data2: 0xfd6d,
    data3: 0x11d0,
    data4: [0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90],
};

/// Defined in `shobjidl_core.h`.
pub const IID_ITaskbarList2: GUID = GUID {
    data1: 0x602d4995,
    data2: 0xb13a,
    data3: 0x429b,
    data4: [0xa6, 0x6e, 0x19, 0x35, 0xe4, 0x4f, 0x43, 0x17],
};
