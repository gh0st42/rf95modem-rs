use core::convert::TryFrom;
use std::convert::TryInto;
use std::io::BufReader;
use std::io::{self, BufRead, Write};
use std::time::Duration;

use serialport::prelude::*;
use serialport::SerialPortType;

pub mod loradev;

use loradev::{LoRaChannels, RF95LoraDevice, RxPacket, Status, ModemConfig};

// Convert byte slice into a hex string
fn hexify(buf: &[u8]) -> String {
    let mut hexstr = String::new();
    for &b in buf {
        hexstr.push_str(&format!("{:02x?}", b));
    }
    hexstr
}

// Convert a hex string into a byte vector
fn unhexify(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}


#[derive(Default)]
pub struct RF95modem {
    settings: SerialPortSettings,
    device: String,
    serial_fd: Option<Box<dyn SerialPort>>,
    reader: Option<Box<dyn BufRead + Send>>,
}

impl Clone for RF95modem {
    fn clone(&self) -> Self {
        if self.serial_fd.is_none() {
            RF95modem {
                settings: self.settings,
                device: self.device.to_owned(),
                serial_fd: None,
                reader: None,
            }
        } else {
            RF95modem {
                settings: self.settings,
                device: self.device.to_owned(),
                serial_fd: Some(self.serial_fd.as_ref().unwrap().try_clone().unwrap()),
                reader: None,
            }
        }
    }
}
impl RF95LoraDevice for RF95modem {
    /// Explicitly open serial device.
    fn open(&mut self) -> Result<(), serialport::Error> {
        self.serial_fd = Some(serialport::open_with_settings(
            &self.device,
            &self.settings,
        )?);
        Ok(())
    }
    /// Get current configuration of modem firmware.
    /// Device must be opened first!
    fn config(&mut self) -> Result<Status, serialport::Error> {
        if self.serial_fd.is_none() {
            self.open()?;
        }
        self.raw_write("AT+INFO\n")?;
        let mut status = Status::new();
        while let line = self.read_line()? {
            if let Some(res) = self.match_split(&line, "firmware") {
                status.version = res;
            }
            if let Some(res) = self.match_split(&line, "max pkt size") {
                status.max_pkt_size = res.parse().unwrap();
            }
            if let Some(res) = self.match_split(&line, "frequency") {
                status.frequency = res.parse().unwrap();
            }
            if let Some(res) = self.match_split(&line, "rx listener") {
                status.rx_listener = res.parse::<usize>().unwrap() == 1;
            }
            if let Some(res) = self.match_split(&line, "rx bad") {
                status.rx_bad = res.parse().unwrap();
            }
            if let Some(res) = self.match_split(&line, "rx good") {
                status.rx_good = res.parse().unwrap();
            }
            if let Some(res) = self.match_split(&line, "tx good") {
                status.tx_good = res.parse().unwrap();
            }
            if let Some(res) = self.match_split(&line, "modem config") {
                let code: usize = res.split('|').nth(0).unwrap().trim().parse().unwrap();
                status.config = code.try_into().unwrap();
            }
            if line.starts_with("+OK") {
                break;
            }
        }
        Ok(status)
    }
    /// Set frequency on rf95modem.
    ///
    /// Device must be opened first!
    fn set_frequency(&mut self, freq: f32) -> Result<(), serialport::Error> {
        if self.serial_fd.is_none() {
            self.open()?;
        }
        let cmd_str = format!("AT+FREQ={}\n", freq);
        self.raw_write(&cmd_str)?;
        let res = self.expect("+FREQ");
        if res.is_ok() {
            Ok(())
        } else {
            Err(res.unwrap_err())
        }
    }
    /// Set config mode on rf95modem.
    ///
    /// Device must be opened first!
    fn set_mode(&mut self, mode: ModemConfig) -> Result<(), serialport::Error> {
        if self.serial_fd.is_none() {
            self.open()?;
        }
        let cmd_str = format!("AT+MODE={}\n", mode as isize);
        self.raw_write(&cmd_str)?;
        let res = self.expect("+OK");
        if res.is_ok() {
            Ok(())
        } else {
            Err(res.unwrap_err())
        }
    }
    /// Send data via configured serial device.
    fn send_data(&mut self, data: Vec<u8>) -> Result<usize, serialport::Error> {
        if self.serial_fd.is_none() {
            self.open()?;
        }
        let cmd_str = format!("AT+TX={}\n", hexify(&data));
        self.raw_write(&cmd_str)?;
        let result = self.read_line()?;
        let fields: Vec<&str> = result.split_ascii_whitespace().collect();
        if result.starts_with("+SENT ") && fields.len() == 3 {
            let bytes_sent: usize = fields[1].parse().unwrap();
            if bytes_sent == data.len() {
                Ok(bytes_sent)
            } else {
                Err(serialport::Error::new(
                    serialport::ErrorKind::InvalidInput,
                    "Number of bytes sent not matching input length.".to_string(),
                ))
            }
        } else {
            Err(serialport::Error::new(
                serialport::ErrorKind::InvalidInput,
                "Unexpected response from modem while sending.".to_string(),
            ))
        }
    }

    /// Read a packet from the modem.
    fn read_packet(&mut self) -> Result<RxPacket, serialport::Error> {
        let input_line = self.expect("+RX ")?;

        if let Ok(rxp) = RxPacket::try_from(input_line.as_str()) {
            Ok(rxp)
        } else {
            Err(serialport::Error::new(
                serialport::ErrorKind::InvalidInput,
                "Error decoding packet.",
            ))
        }
    }
    /// Read a raw line from the serial device.
    ///
    /// **This method will probably made private in the future.**
    fn read_line(&mut self) -> Result<String, serialport::Error> {
        if self.serial_fd.is_none() {
            self.open()?;
        }
        if self.reader.is_none() {
            let sfd = self.serial_fd.as_mut().unwrap();
            self.reader = Some(Box::new(BufReader::new(sfd.try_clone().unwrap())));
        }

        let mut serial_str = String::new();
        match self.reader.as_mut().unwrap().read_line(&mut serial_str) {
            Ok(_) => Ok(serial_str),
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => Err(serialport::Error::new(
                serialport::ErrorKind::Io(io::ErrorKind::TimedOut),
                "Read timeout",
            )),
            Err(ref e) => Err(serialport::Error::new(
                serialport::ErrorKind::Io(e.kind()),
                format!("{:?}", e),
            )),
        }
    }
}
impl RF95modem {
    /// Create a new RF95modem at specified device path and baud rate.
    ///
    /// The device is not opened automatically!
    ///
    /// Receive timeout is configured to 1 second.
    pub fn new(device: &str, baud_rate: u32) -> Self {
        let mut settings: SerialPortSettings = Default::default();
        settings.timeout = Duration::from_secs(1);
        settings.baud_rate = baud_rate;
        RF95modem {
            settings,
            device: device.to_string(),
            serial_fd: None,
            reader: None,
        }
    }
    fn expect(&mut self, starts_with: &str) -> Result<String, serialport::Error> {
        let result = self.read_line()?;
        if result.starts_with(starts_with) {
            Ok(result)
        } else {
            Err(serialport::Error::new(
                serialport::ErrorKind::InvalidInput,
                "Unexpected result from modem.".to_string(),
            ))
        }
    }
    fn match_split(&self, input: &str, key: &str) -> Option<String> {
        if input.starts_with(key) {
            Some(input.split(':').nth(1)?.trim().into())
        } else {
            None
        }
    }
    /// Write directly to modem via serial interface.
    ///
    /// Device must be opened first!
    ///
    /// **This method will probably made private in the future.**
    pub fn raw_write(&mut self, buf: &str) -> Result<(), serialport::Error> {
        match self.serial_fd.as_mut().unwrap().write_all(buf.as_bytes()) {
            Ok(()) => Ok(()),
            Err(e) => Err(serialport::Error::new(
                serialport::ErrorKind::Io(e.kind()),
                format!("{:?}", e),
            )),
        }
    }
}

/// Returns a default usb serial device for macos or unix.
/// It might not be present or called otherwise depending on
/// the system configuration.
pub fn get_default_usb_serial() -> String {
    if cfg!(target_os = "macos") {
        String::from("/dev/tty.SLAB_USBtoUART")
    } else {
        String::from("/dev/ttyUSB0")
    }
}

/// A little helper function that dumps information about
/// all available serial ports on the system.
pub fn dump_all_serial_ports() {
    if let Ok(ports) = serialport::available_ports() {
        match ports.len() {
            0 => println!("No ports found."),
            1 => println!("Found 1 port:"),
            n => println!("Found {} ports:", n),
        };
        for p in ports {
            println!("  {}", p.port_name);
            match p.port_type {
                SerialPortType::UsbPort(info) => {
                    println!("    Type: USB");
                    println!("    VID:{:04x} PID:{:04x}", info.vid, info.pid);
                    println!(
                        "     Serial Number: {}",
                        info.serial_number.as_ref().map_or("", String::as_str)
                    );
                    println!(
                        "      Manufacturer: {}",
                        info.manufacturer.as_ref().map_or("", String::as_str)
                    );
                    println!(
                        "           Product: {}",
                        info.product.as_ref().map_or("", String::as_str)
                    );
                }
                SerialPortType::BluetoothPort => {
                    println!("    Type: Bluetooth");
                }
                SerialPortType::PciPort => {
                    println!("    Type: PCI");
                }
                SerialPortType::Unknown => {
                    println!("    Type: Unknown");
                }
            }
        }
    } else {
        print!("Error listing serial ports");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
