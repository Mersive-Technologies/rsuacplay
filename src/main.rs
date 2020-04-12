extern crate libusb_sys as ffi;

use core::mem;
use std::os::raw::{c_uchar, c_uint};
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use ffi::{libusb_context, libusb_device_handle, LIBUSB_SUCCESS, libusb_transfer, LIBUSB_TRANSFER_TYPE_ISOCHRONOUS};

const SPKR_EP: u8 = 1;
const ID_VENDOR: u16 = 0x046d;
const ID_PRODUCT: u16 = 0x0867;
const SAMPLE_RATE: u32 = 32000;
const CTRL_IFACE: i32 = 0;
const SPKR_IFACE: i32 = 2;
const ISO_PKT_PER_FRAME: u32 = 6;
const BYTES_PER_ISO_PKT: u32 = 128;
const TIMEOUT: c_uint = 1000;
const A4: f32 = 440.0f32;
const CHANNEL_CNT: u32 = 2;
const TAU: f32 = std::f32::consts::PI * 2.0f32;
const BUFFER_SZ: usize = (BYTES_PER_ISO_PKT * ISO_PKT_PER_FRAME) as usize;

type SampleSize = i16;

// pros: no forward declarations, no h files, cargo easier thn cmake, sane int types, made me think about allocation
// cons: some pointer ceremony, more docs and less S/O, made me think about allocation

struct PlayState {
    samples_played: Mutex<u32>,
    buffers: [[SampleSize; BUFFER_SZ]; 3],
    xfers: [*mut libusb_transfer; 3],
}

impl PlayState {
    fn new() -> PlayState {
        PlayState {
            samples_played: Mutex::new(0),
            buffers: [[0i16; BUFFER_SZ]; 3],
            xfers: [null_mut(); 3],
        }
    }
}

fn main() {
    unsafe {
        // init
        let mut ctx: *mut libusb_context = null_mut();
        ffi::libusb_init(&mut ctx);
        if ctx == null_mut() {
            panic!("Couldn't init usb");
        }

        let dev_handle = ffi::libusb_open_device_with_vid_pid(ctx, ID_VENDOR, ID_PRODUCT);
        if dev_handle == null_mut() {
            panic!("Couldn't open device");
        }

        detach_kernel(dev_handle, CTRL_IFACE);
        detach_kernel(dev_handle, SPKR_IFACE);

        claim_iface(dev_handle, CTRL_IFACE);
        claim_iface(dev_handle, SPKR_IFACE);

        if ffi::libusb_set_interface_alt_setting(dev_handle, SPKR_IFACE, 1) != LIBUSB_SUCCESS {
            panic!("Can't enable speaker!");
        }

        // play
        let mut state = PlayState::new();
        for i in 0..3 {
            state.xfers[i] = ffi::libusb_alloc_transfer(ISO_PKT_PER_FRAME as i32);
            fill_xfer(&state, i, dev_handle);
        }
    }
}

unsafe fn fill_xfer(play_state: &PlayState, buff_idx: usize, dev_handle: *mut libusb_device_handle) {
    let mut samples_played = play_state.samples_played.lock().unwrap();

    let byte_cnt = BYTES_PER_ISO_PKT * ISO_PKT_PER_FRAME;
    let sample_cnt = byte_cnt / CHANNEL_CNT / mem::size_of::<SampleSize>() as u32;
    let max_samp_vol = SampleSize::max_value();
    for i in 0..sample_cnt {
        let seconds = (*samples_played + i) as f32 / SAMPLE_RATE as f32;
        let cycles = A4 * seconds;
        let samp = (cycles * TAU).sin() * (max_samp_vol as f32);
        let mut buff = play_state.buffers[buff_idx];
        buff[i as usize] = samp as i16;
    }
    *samples_played += sample_cnt;

    let buff: *const [SampleSize; BUFFER_SZ] = &play_state.buffers[buff_idx];
    let xfer = play_state.xfers[buff_idx];
    (*xfer).dev_handle = dev_handle;
    (*xfer).endpoint = SPKR_EP;
    (*xfer).transfer_type = LIBUSB_TRANSFER_TYPE_ISOCHRONOUS;
    (*xfer).timeout = TIMEOUT;
    (*xfer).buffer = buff as *mut c_uchar;
    (*xfer).length = byte_cnt as i32;
    (*xfer).num_iso_packets = ISO_PKT_PER_FRAME as i32;
    (*xfer).callback = xfer_complete;
//    play_state.xfers[buff_idx].user_data = this;

    for i in 0..(*xfer).num_iso_packets {
        (*xfer).iso_packet_desc[i as usize].length = BYTES_PER_ISO_PKT;
    }
}

extern "C" fn xfer_complete(transfer: *mut libusb_transfer) {}

unsafe fn detach_kernel(dev_handle: *mut libusb_device_handle, iface_id: i32) {
    if ffi::libusb_kernel_driver_active(dev_handle, iface_id) != 1 {
        return;
    }
    if ffi::libusb_detach_kernel_driver(dev_handle, iface_id) != 0 {
        panic!("failed to detach");
    }
}

unsafe fn claim_iface(dev_handle: *mut libusb_device_handle, iface_id: i32) {
    if ffi::libusb_claim_interface(dev_handle, iface_id) != LIBUSB_SUCCESS {
        panic!("Couldn't claim interface");
    }
}
