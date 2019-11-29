use lora_modem_hal::LoraModemDevice;
use rf95modem::{dump_all_serial_ports, get_default_usb_serial, RF95modem};

fn main() {
    let device = get_default_usb_serial();
    let mut modem = RF95modem::new(&device, 115_200);

    println!("rf95modem sender example");
    dump_all_serial_ports();

    modem.open().unwrap();
    dbg!(modem.config());
}
