use rf95modem::{dump_all_serial_ports, RF95modem, get_default_usb_serial};
use rf95modem::loradev::RF95LoraDevice;

fn main() {
    let device = get_default_usb_serial();
    let mut modem = RF95modem::new(&device, 115_200);

    println!("rf95modem sender example");
    dump_all_serial_ports();

    modem.open().unwrap();
    dbg!(modem.send_data(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_vec()));
}
