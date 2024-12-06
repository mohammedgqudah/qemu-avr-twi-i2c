use core::ptr::NonNull;
use std::{
    ffi::{c_uint, c_void, CStr},
    ptr::addr_of_mut,
};

use qemu_api::{
    bindings::{
        hwaddr, i2c_end_transfer, i2c_init_bus, i2c_send, memory_region_init_io, qemu_irq,
        qemu_set_irq, sysbus_init_irq, sysbus_init_mmio, DeviceState, I2CBus, MemoryRegion, Object,
        ObjectClass, SysBusDevice,
    },
    c_str,
    definitions::ObjectImpl,
    device_class::TYPE_SYS_BUS_DEVICE,
};
use qemu_api_macros::Object;

use crate::{memory_ops::TWI_I2C_OPS, registers};

#[derive(Debug, Object, qemu_api_macros::offsets)]
#[repr(C)]
pub struct TWI_I2CState {
    pub parent_obj: SysBusDevice,
    pub iomem: MemoryRegion,
    pub someprop: char,
    pub bus: *mut I2CBus,
    pub irq: qemu_irq,

    pub twbr: registers::TWBR,
    pub twsr: registers::TWSR,
    pub twar: registers::TWAR,
    pub twdr: registers::TWDR,
    pub twcr: registers::TWCR,

    pub enabled: bool,
}

impl ObjectImpl for TWI_I2CState {
    type Class = TWI_I2CClass;
    const TYPE_INFO: qemu_api::bindings::TypeInfo = qemu_api::type_info! { Self };
    const TYPE_NAME: &'static CStr = crate::TYPE_TWI_I2C;
    const PARENT_TYPE_NAME: Option<&'static CStr> = Some(TYPE_SYS_BUS_DEVICE);
    const ABSTRACT: bool = false;
    const INSTANCE_INIT: Option<unsafe extern "C" fn(obj: *mut Object)> = Some(twi_i2c_init);
    const INSTANCE_POST_INIT: Option<unsafe extern "C" fn(obj: *mut Object)> = None;
    const INSTANCE_FINALIZE: Option<unsafe extern "C" fn(obj: *mut Object)> = None;
}

impl TWI_I2CState {
    /// Initializes a pre-allocated, unitialized instance of `TWI_I2CState`.
    ///
    /// # Safety
    ///
    /// `self` must point to a correctly sized and aligned location for the
    /// `TWI_I2CState` type. It must not be called more than once on the same
    /// location/instance. All its fields are expected to hold unitialized
    /// values with the sole exception of `parent_obj`.
    pub fn init(&mut self) {
        println!("init twi");
        let device = addr_of_mut!(*self).cast::<DeviceState>();
        let sbd = addr_of_mut!(*self).cast::<SysBusDevice>();
        unsafe {
            sysbus_init_mmio(sbd, addr_of_mut!(self.iomem));
            sysbus_init_irq(sbd, &mut self.irq);
            self.bus = i2c_init_bus(device, c_str!("i2c-bus").as_ptr());
        }
    }

    pub fn realize(&mut self) {
        println!("realize twi");
        unsafe {
            memory_region_init_io(
                addr_of_mut!(self.iomem),
                addr_of_mut!(*self).cast::<Object>(),
                &TWI_I2C_OPS,
                addr_of_mut!(*self).cast::<c_void>(),
                Self::TYPE_INFO.name,
                0x6, // TODO: What should the region size be?
            );
        }
    }

    /// Reset the TWI controller registers.
    ///
    /// Based on "Register Description" in the ATmega640/1280/1281/2560/2561
    /// Datasheet.
    pub fn reset(&mut self) {
        println!("reset");
        self.twbr = 0.into();
        self.twsr = 0b11111000.into();
        self.twar = 0b11111110.into();
        self.twdr = 0xFF.into();
        self.twcr = 0.into();
    }

    pub fn read(&mut self, offset: hwaddr, _size: c_uint) -> u64 {
        // TODO: reset based on the Atmel docs
        //println!("read address: {}: size: {}", offset, size);
        match offset {
            0 => {
                println!("reg 0");
            }
            1 => {
                let x: u8 = self.twsr.into();
                println!("TWSR: {}", x as u64);
                return x as u64;
            }
            2 => {
                println!("reg 1");
            }
            3 => {
                println!("reg 3");
            }
            4 => {
                println!("reg 4");
                let x: u8 = self.twcr.into();
                return x as u64;
            }
            5 => {
                println!("reg 5");
            }
            _ => {
                eprintln!("bad offset");
            }
        }
        0xFF
    }

    pub fn write(&mut self, address: hwaddr, data: u8) {
        println!("write address: {}: data: {}", address, data);
        match address {
            0 => {
                println!("setting TWI bit rate");
                let _r = registers::TWBR::from(data);
            }
            1 => {
                // TODO: handle the first two bits.
                let _r = registers::TWSR::from(data);
            }
            2 => {
                let r = registers::TWAR::from(data);
                println!("{:#?}", r);
            }
            3 => {
                self.twdr = registers::TWDR::from(data);
                println!("{:#?}", self.twdr);
                println!("data: {}", u8::from(self.twdr) as char);
                self.write_data();
            }
            4 => {
                let r = registers::TWCR::from(data);
                println!("{:#?}", r);
                // TODO: if this bit is reset, terminate all on going trasmissions
                self.enabled = r.twen();
                self.twcr.set_twint(r.twint());

                if r.twsta() {
                    self.twcr.set_twsta(true);
                }

                if r.twsto() {
                    self.stop();
                } else if r.twint() && r.twen() {
                    // TODO: The global interrupt must be enabled SREG (I)

                    // This is a new transaction
                    if !self.twcr.twen() {
                        // TODO: don't touch the first two? bits when setting the status
                        self.twsr = registers::TW_START.into();
                        self.twcr.set_twen(true);
                    }

                    unsafe {
                        qemu_set_irq(self.irq, 1);
                    };
                    self.twcr.set_twint(true); // Fake TWI
                }
            }
            _ => {
                eprintln!("bad offset");
            }
        }
    }

    /// Handle a START condition.
    fn _start(&self) {}

    /// Handle a STOP condition.
    fn stop(&mut self) {
        unsafe {
            i2c_end_transfer(self.bus);
        };
        self.twcr.set_twsto(false); // report that STOP has executed on the bus.
        self.twcr.set_twint(false); // TODO: according to twi_stop
                                    // in the arduino library, confirm
                                    // in
                                    // the data sheet
    }
    fn write_data(&mut self) {
        // report that you cannot write when twint is low
        if !self.twcr.twint() {
            self.twcr.set_twwc(true);
            return;
        }

        if self.twcr.twsta() {
            // If the START bit was set, then this is the first data in the transaction
            // and it contains the sla+w (slave address + w bit)
            self.twcr.set_twsta(false);
            match i2c_start_transfer(self.bus, u8::from(self.twdr) >> 1, false) {
                Ok(_) => {
                    self.set_status(registers::TW_MT_SLA_ACK);
                }
                Err(_) => {
                    // TODO: Whats the correct status here?
                    // TODO: Uncomment after implementing a device. Because i2c_start_transfer
                    // fails without a device
                    //self.set_status(registers::TW_BUS_ERROR);
                    self.set_status(registers::TW_MT_SLA_ACK);
                }
            }
        } else {
            unsafe { i2c_send(self.bus, self.twdr.into()) };
            self.set_status(registers::TW_MT_DATA_ACK);
        }
    }

    /// Set the status bits in TWSR.
    fn set_status(&mut self, status: u8) {
        // TODO: only modify the last 5 bits.
        self.twsr = status.into();
    }
}

// TODO: move somewhere else
// TODO: Should this be safe or unsafe?
fn i2c_start_transfer(bus: *mut I2CBus, address: u8, is_recv: bool) -> Result<(), ()> {
    println!("Starting a transfer @ address: {}", address);
    let result = unsafe { qemu_api::bindings::i2c_start_transfer(bus, address, is_recv) };
    if result > 0 {
        Err(())
    } else {
        Ok(())
    }
}

#[repr(C)]
pub struct TWI_I2CClass {}

impl qemu_api::definitions::Class for TWI_I2CClass {
    const CLASS_INIT: Option<unsafe extern "C" fn(class: *mut ObjectClass, data: *mut c_void)> =
        Some(crate::device_class::twi_i2c_class_init);
    const CLASS_BASE_INIT: Option<
        unsafe extern "C" fn(class: *mut ObjectClass, data: *mut c_void),
    > = None;
}

/// # Safety
///
/// We expect the FFI user of this function to pass a valid pointer, that has
/// the same size as [`TWI_I2CState`]. We also expect the device is
/// readable/writeable from one thread at any time.
pub unsafe extern "C" fn twi_i2c_init(obj: *mut Object) {
    unsafe {
        debug_assert!(!obj.is_null());
        let mut state = NonNull::new_unchecked(obj.cast::<TWI_I2CState>());
        state.as_mut().init();
    }
}
