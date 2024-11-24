use qemu_api::bindings::*;

/// RAII: The guard returned by `.start()` to send data.
pub struct Transaction<'s> {
    slave: &'s Slave,
}

impl Transaction<'_> {
    pub fn send(&self) {
        //i2c_start_send();
    }
    fn end(&self) {
        //i2c_end_transfer(); 
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        self.end();
    }
}

/// TODO: a struct for registering I2C devices "slaves"
/// TODO: Should i use the term device instead?
pub struct Slave {
    address: u64,
    bus: I2CBus,
}
impl Slave {
}
