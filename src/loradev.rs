use serialport::prelude::*;
use std::convert::TryFrom;

/// Predefined LoRa channels and frequencies
pub enum LoRaChannels {
    // 868MHz EU TTN Channels 1-9
    Ch01_868 = 86810,
    Ch02_868 = 86830,
    Ch03_868 = 86850,
    Ch04_868 = 86710,
    Ch05_868 = 86730,
    Ch06_868 = 86750,
    Ch07_868 = 86770,
    Ch08_868 = 86790,
    Ch09_868 = 86880,
    // Further 868MHz EU channels 10-17 from https://www.rfwireless-world.com/Tutorials/LoRa-channels-list.html
    Ch10_868 = 86520,
    Ch11_868 = 86550,
    Ch12_868 = 86580,
    Ch13_868 = 86610,
    Ch14_868 = 86640,
    Ch15_868 = 86670,
    Ch16_868 = 86700,
    Ch17_868 = 86800,

    // 915MHz US channels 0-12 https://www.rfwireless-world.com/Tutorials/LoRa-channels-list.html
    Ch00_900 = 90308,
    Ch01_900 = 90524,
    Ch02_900 = 90740,
    Ch03_900 = 90956,
    Ch04_900 = 91172,
    Ch05_900 = 91388,
    Ch06_900 = 91604,
    Ch07_900 = 91820,
    Ch08_900 = 92036,
    Ch09_900 = 92252,
    Ch10_900 = 92468,
    Ch11_900 = 92684,
    Ch12_900 = 91500,
}

/// A LoRa packet received from the modem
#[derive(Debug)]
pub struct RxPacket {
    /// Signal strength
    pub rssi: i16,
    /// Signal-to-Noise ratio
    pub snr: i16,
    /// Received binary data
    pub data: Vec<u8>,
}
impl TryFrom<&str> for RxPacket {
    type Error = &'static str;

    fn try_from(item: &str) -> Result<Self, Self::Error> {
        let item_payload = if &item[0..4] == "+RX " {
            &item[4..]
        } else {
            item
        };
        let fields: Vec<&str> = dbg!(item_payload.trim()).split(',').collect();
        if fields.len() != 4 {
            return Err("+RX output from modem has unexpected length!");
        }
        let len: usize = fields[0].parse().unwrap();
        let data = crate::unhexify(fields[1]).unwrap();
        if data.len() != len {
            return Err("+RX payload length not matching actual payload!");
        }
        let rssi: i16 = fields[2].parse().unwrap();
        let snr: i16 = fields[3].parse().unwrap();
        /*let recv_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();*/

        Ok(RxPacket {
            rssi,
            snr,
            data,
            //recv_time,
        })
    }
}

/// Default LoRa modem configs
#[derive(Debug)]
pub enum ModemConfig {
    /// Medium Range (Default)
    MediumBw125Cr45Sf128Crc = 0,
    /// Fast transmission, short range
    FastShortBw500Cr45Sf128Crc = 1,
    /// Slow transmission, long range
    SlowLongBw3125Cr48Sf512Crc = 2,
    /// Slow transmission, long range
    SlowLongBw125Cr48Sf4096Crc = 3,
}

impl TryFrom<usize> for ModemConfig {
    type Error = &'static str;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value == 0 {
            Ok(ModemConfig::MediumBw125Cr45Sf128Crc)
        } else if value == 1 {
            Ok(ModemConfig::FastShortBw500Cr45Sf128Crc)
        } else if value == 2 {
            Ok(ModemConfig::SlowLongBw3125Cr48Sf512Crc)
        } else if value == 3 {
            Ok(ModemConfig::SlowLongBw125Cr48Sf4096Crc)
        } else {
            Err("Unknown modem config code!")
        }
    }
}

/// Current rf95modem status
#[derive(Debug)]
pub struct Status {
    /// firmware version running on modem
    pub version: String,
    /// current LoRa config settings
    pub config: ModemConfig,
    /// maximum packet size supported
    pub max_pkt_size: usize,
    /// current frequency configured on modem
    pub frequency: f32,
    /// receiving of incoming packets activated
    pub rx_listener: bool,

    /// number of receive errors
    pub rx_bad: usize,
    /// number of successfully received packets
    pub rx_good: usize,
    /// number of successfully transmitted packets
    pub tx_good: usize,
}

impl Default for Status {
    fn default() -> Self {
        Self::new()
    }
}

impl Status {
    pub fn new() -> Self {
        Status {
            version: "0.0".to_string(),
            config: ModemConfig::MediumBw125Cr45Sf128Crc,
            max_pkt_size: 0,
            frequency: 0.0,
            rx_listener: false,
            rx_bad: 0,
            rx_good: 0,
            tx_good: 0,
        }
    }
}

pub trait RF95LoraDevice {
    /// Explicitly open serial device.
    fn open(&mut self) -> Result<(), serialport::Error>;
    /// Set channel on rf95modem.
    fn set_channel(&mut self, channel: LoRaChannels) -> Result<(), serialport::Error> {
        let freq = (channel as i32) as f32 / 100.0;
        self.set_frequency(freq)
    }
    /// Set frequency on rf95modem.
    fn set_frequency(&mut self, freq: f32) -> Result<(), serialport::Error>;
    /// Get current configuration of modem firmware.
    fn config(&mut self) -> Result<Status, serialport::Error>;
    /// Set config mode on rf95modem.
    fn set_mode(&mut self, mode: ModemConfig) -> Result<(), serialport::Error>;
    /// Send data via configured serial device.
    fn send_data(&mut self, data: Vec<u8>) -> Result<usize, serialport::Error>;
    /// Read a packet from the modem.
    fn read_packet(&mut self) -> Result<RxPacket, serialport::Error>;
    /// Read a raw line from the serial device.
    fn read_line(&mut self) -> Result<String, serialport::Error>;
}
