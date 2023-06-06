extern crate min_rs as min;
extern crate log;

use std::io::prelude::*;
use std::io;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use log::{debug, trace};
use serialport::SerialPort;
use spidev::{Spidev, SpidevTransfer};
use std::thread;

pub const REG_OPMODE: u8 = 0x01;
pub const REG_FIFO: u8 = 0x00;
pub const REG_PA_CONFIG: u8 = 0x09;
pub const REG_FIFO_ADDR_PTR: u8 = 0x0D;
pub const REG_FIFO_TX_BASE_AD: u8 = 0x0E;
pub const REG_FIFO_RX_BASE_AD: u8 = 0x0F;
pub const REG_RX_NB_BYTES: u8 = 0x13;
pub const REG_FIFO_RX_CURRENT_ADDR: u8 = 0x10;
pub const REG_IRQ_FLAGS: u8 = 0x12;
pub const REG_PKT_RSSI_VALUE: u8 = 0x1A;
pub const REG_RSSI_VALUE: u8 = 0x1B;
pub const REG_DIO_MAPPING_1: u8 = 0x40;
pub const REG_DIO_MAPPING_2: u8 = 0x41;
pub const REG_MODEM_CONFIG: u8 = 0x1D;
pub const REG_MODEM_CONFIG2: u8 = 0x1E;
pub const REG_MODEM_CONFIG3: u8 = 0x26;
pub const REG_SYMB_TIMEOUT_LSB: u8 = 0x1F;
pub const REG_PKT_SNR_VALUE: u8 = 0x19;
pub const REG_PREAMBLE_MSB: u8 = 0x20;
pub const REG_PREAMBLE_LSB: u8 = 0x21;
pub const REG_PAYLOAD_LENGTH: u8 = 0x22;
pub const REG_IRQ_FLAGS_MASK: u8 = 0x11;
pub const REG_MAX_PAYLOAD_LENGTH: u8 = 0x23;
pub const REG_HOP_PERIOD: u8 = 0x24;
pub const REG_SYNC_WORD: u8 = 0x39;
pub const REG_DIO_MAPPING1: u8 = 0x40;
pub const REG_VERSION: u8 = 0x42;
pub const REG_PA_DAC: u8 = 0x4d;

// LOW NOISE AMPLIFIER
pub const REG_LNA: u8 = 0x0C;
pub const LNA_MAX_GAIN: u8 = 0x23;
pub const LNA_OFF_GAIN: u8 = 0x00;
pub const LNA_LOW_GAIN: u8 = 0x20;
// FRF
pub const REG_FRF_MSB: u8 = 0x06;
pub const REG_FRF_MID: u8 = 0x07;
pub const REG_FRF_LSB: u8 = 0x08;
// PA_DAC
pub const PA_DAC_DISABLE: u8 = 0x04;
pub const PA_DAC_ENABLE: u8 = 0x07;
// PA_CONFIG
pub const PA_SELECT: u8 = 0x80;
// OP_MODE
pub const SX72_LONG_RANGE_MODE: u8 = 0x80;
pub const SX72_MODE_SLEEP: u8 = 0x00;
pub const SX72_MODE_STANDBY: u8 = 0x01;
pub const SX72_MODE_TX: u8 = 0x03;
pub const SX72_MODE_RX_CONTINUOS: u8 = 0x05;
// REG_IRQ_FLAGS
pub const RX_TIMEOUT: u8 = 0x80;
pub const RX_DONE: u8 = 0x40;
pub const PAYLOAD_CRC_ERROR: u8 = 0x20;
pub const VALID_HEADER: u8 = 0x10;
pub const TX_DONE: u8 = 0x08;
pub const CAD_DONE: u8 = 0x04;
pub const FHSS_CHANGE_CHANNEL: u8 = 0x02;
pub const CAD_DETECTED: u8 = 0x01;

pub const FREQ: u32 = 434000000; // 434 Mhz

enum ModemConfigChoice {
    Bw125Cr45Sf128, //< Bw = 125 kHz, Cr = 4/5, Sf = 128chips/symbol, CRC on. Default medium range
    Bw500Cr45Sf128, //< Bw = 500 kHz, Cr = 4/5, Sf = 128chips/symbol, CRC on. Fast+short range
    Bw31_25Cr48Sf512, //< Bw = 31.25 kHz, Cr = 4/8, Sf = 512chips/symbol, CRC on. Slow+long range
    Bw125Cr48Sf4096, //< Bw = 125 kHz, Cr = 4/8, Sf = 4096chips/symbol, CRC on. Slow+long range
}

#[derive(Debug,Copy,Clone)]
pub struct FileIf;

impl FileIf {
    pub fn read(self, filename: &str) -> Result<String, ()> {
        match std::fs::read_to_string(filename) {
            Ok(msg) => {
                match std::fs::write(filename, "") {
                    _ => {},
                };
                Ok(msg)
            },
            Err(_err) => Err(()),
        }
    }
}

pub struct HwIf {
    port: RefCell<Box<dyn SerialPort>>,
    name: String,
    tx_space_avaliable: u16,
    output: Arc<Mutex<String>>,
}

impl HwIf {
    pub fn new(port: Box<dyn SerialPort>, name: String, tx_space_avaliable: u16) -> Self {
        HwIf {
            port: RefCell::new(port),
            name: name,
            tx_space_avaliable: tx_space_avaliable,
            output: Arc::new(Mutex::new(String::from(""))),
        }
    }

    fn available_for_write(&self) -> u16 {
        self.tx_space_avaliable
    }

    fn tx(&self, byte: u8) {
        let mut output = self.output.lock().unwrap();
        output.push_str(format!("0x{:02x} ", byte).as_str());
        let mut port = self.port.borrow_mut();
        match port.write(&[byte]) {
            Ok(_) => {},
            Err(e) => {
                debug!(target: self.name.as_str(), "{}", e);
            },
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut port = self.port.borrow_mut();
        match port.read(&mut buf[..]) {
            Ok(n) => Ok(n),
            _ => Err(()),
        }
    }
}

impl min::Interface for HwIf {
    fn tx_start(&self) {
        let mut output = self.output.lock().unwrap();
        output.clear();
        output.push_str(format!("send frame: [ ").as_str());
    }
    
    fn tx_finished(&self) {
        let mut output = self.output.lock().unwrap();
        output.push_str(format!("]").as_str());
        trace!(target: self.name.as_str(), "{}", output);
    }
    fn tx_space(&self) -> u16 {
        self.available_for_write()
    }
    
    fn tx_byte(&self, _min_port: u8, byte: u8) {
        self.tx(byte);
    }
}


#[derive(Debug, Copy, Clone)]
pub struct SpiIf;

impl SpiIf {
    fn read_register(self, spi: &mut Spidev, addr: u8) -> io::Result<u8> {
        let mut rx_buf = [0_u8; 2];
        let tx_buf = [addr & 0x7f, 0];
        let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
        spi.transfer(&mut transfer)?;

        Ok(rx_buf[1])
    }

    fn write_register(self, spi: &mut Spidev, addr: u8, value: u8) -> io::Result<()> {
        spi.write(&[addr | 0x80, value])?;

        Ok(())
    }

    fn set_modem_config(self, spi: &mut Spidev, config: ModemConfigChoice) -> io::Result<()> {
        match config {
            ModemConfigChoice::Bw125Cr45Sf128 => {
                self.write_register(spi, REG_MODEM_CONFIG, 0x72)?;
                self.write_register(spi, REG_MODEM_CONFIG2, 0x74)?;
                self.write_register(spi, REG_MODEM_CONFIG3, 0x00)?;
                Ok(())
            }
            ModemConfigChoice::Bw500Cr45Sf128 => {
                self.write_register(spi, REG_MODEM_CONFIG, 0x92)?;
                self.write_register(spi, REG_MODEM_CONFIG2, 0x74)?;
                self.write_register(spi, REG_MODEM_CONFIG3, 0x00)?;
                Ok(())
            }
            ModemConfigChoice::Bw31_25Cr48Sf512 => {
                self.write_register(spi, REG_MODEM_CONFIG, 0x48)?;
                self.write_register(spi, REG_MODEM_CONFIG2, 0x94)?;
                self.write_register(spi, REG_MODEM_CONFIG3, 0x00)?;
                Ok(())
            }
            ModemConfigChoice::Bw125Cr48Sf4096 => {
                self.write_register(spi, REG_MODEM_CONFIG, 0x78)?;
                self.write_register(spi, REG_MODEM_CONFIG2, 0xc4)?;
                self.write_register(spi, REG_MODEM_CONFIG3, 0x08)?;
                Ok(())
            }
        }
    }

    fn set_preamble_length(self, spi: &mut Spidev, length: u16) -> io::Result<()> {
        self.write_register(spi, REG_PREAMBLE_MSB, (length >> 8) as u8)?;
        self.write_register(spi, REG_PREAMBLE_LSB, (length & 0xff) as u8)?;
        Ok(())
    }

    fn set_frequency(self, spi: &mut Spidev, freq: u32) -> io::Result<()> {
        // Frf = FRF / FSTEP
        let frf: u64 = ((freq as u64) << 19) / 32000000;
        self.write_register(spi, REG_FRF_MSB, (frf >> 16) as u8)?;
        self.write_register(spi, REG_FRF_MID, (frf >> 8) as u8)?;
        self.write_register(spi, REG_FRF_LSB, frf as u8)?;

        Ok(())
    }

    fn set_tx_power(self, spi: &mut Spidev, power: u8) -> io::Result<()> {
        let mut p = power;
        if power > 23 {
            p = 23;
        }
        if power < 5 {
            p = 5;
        }
        // For PA_DAC_ENABLE, manual says '+20dBm on PA_BOOST when OutputPower=0xf'
        // PA_DAC_ENABLE actually adds about 3dBm to all power levels. We will us it
        // for 21, 22 and 23dBm
        if p > 20 {
            self.write_register(spi, REG_PA_DAC, PA_DAC_ENABLE)?;
            p -= 3;
        } else {
            self.write_register(spi, REG_PA_DAC, PA_DAC_DISABLE)?;
        }

        self.write_register(spi, REG_PA_CONFIG, PA_SELECT | (p - 5))?;

        Ok(())
    }

    fn set_mode_rx(self, spi: &mut Spidev) -> io::Result<()> {
        self.write_register(spi, REG_OPMODE, SX72_MODE_RX_CONTINUOS)?;
        self.write_register(spi, REG_DIO_MAPPING1, 0x00)?;

        Ok(())
    }

    fn setup_lora(self, spi: &mut Spidev) -> io::Result<()> {
        let version = self.read_register(spi, REG_VERSION)?;
        match version {
            0x22 => println!("SX1272 detected, starting."),
            0x12 => println!("SX1276 detected, starting."),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("Unrecognized transceiver(version: 0x{:02X})", version),
                ));
            }
        }

        // Set sleep mode, so we can also set LORA mode:
        self.write_register(spi, REG_OPMODE, SX72_MODE_SLEEP | SX72_LONG_RANGE_MODE)?;
        thread::sleep(std::time::Duration::from_millis(100));
        let op_mode = self.read_register(spi, REG_OPMODE)?;
        if op_mode != (SX72_MODE_SLEEP | SX72_LONG_RANGE_MODE) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("REG_OPMODE(0x{:02X}): 0x{:02X}", REG_OPMODE, op_mode),
            ));
        }

        // Set up FIFO
        // We configure so that we can use the entire 256 byte FIFO for either receive
        // or transmit, but not both at the same time
        self.write_register(spi, REG_FIFO_TX_BASE_AD, 0)?;
        self.write_register(spi, REG_FIFO_RX_BASE_AD, 0)?;

        self.write_register(spi, REG_OPMODE, SX72_MODE_STANDBY)?;

        // Bw = 125 kHz, Cr = 4/8, Sf = 4096chips/symbol, CRC on. Slow+long range
        self.set_modem_config(spi, ModemConfigChoice::Bw125Cr48Sf4096)?;

        self.set_preamble_length(spi, 8)?;
        self.set_frequency(spi, FREQ)?;
        self.set_tx_power(spi, 13)?;

        self.set_mode_rx(spi)?;

        Ok(())
    }

    pub fn init(self, spi: &mut Spidev) -> io::Result<()> {
        self.setup_lora(spi)?;

        Ok(())
    }

    pub fn read(self, spi: &mut Spidev) -> Result<String, ()> {
        let mut buffer: [u8; 256] = [0; 256];
        if let Ok(_) = self.set_mode_rx(spi) {
            if let Ok(irq_flags) = self.read_register(spi, REG_IRQ_FLAGS) {
                if irq_flags & RX_DONE != 0 {
                    if irq_flags & 0x20 == 0x20 {
                        debug!("CRC error");
                        Err(())
                    } else {
                        if let Ok(current_addr) = self.read_register(spi, REG_FIFO_RX_CURRENT_ADDR)
                        {
                            if let Ok(received_count) = self.read_register(spi, REG_RX_NB_BYTES) {
                                if let Ok(_) =
                                    self.write_register(spi, REG_FIFO_ADDR_PTR, current_addr)
                                {
                                    for i in 0..received_count as usize {
                                        buffer[i] = match self.read_register(spi, REG_FIFO) {
                                            Ok(b) => b,
                                            Err(e) => {
                                                debug!("{}", e);
                                                0
                                            }
                                        }
                                    }
                                    // Clear all IRQ flags
                                    match self.write_register(spi, REG_IRQ_FLAGS, 0xff) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            debug!("{}", e);
                                        }
                                    }
                                    if let Ok(s) = String::from_utf8(
                                        buffer[4..received_count as usize].to_vec(),
                                    ) {
                                        Ok(s)
                                    } else {
                                        debug!("get string error");
                                        Err(())
                                    }
                                } else {
                                    debug!(
                                        "write REG_FIFO_ADDR_PTR(0x{:02X}) error",
                                        REG_FIFO_ADDR_PTR
                                    );
                                    Err(())
                                }
                            } else {
                                debug!("read REG_RX_NB_BYTES(0x{:02X}) error", REG_RX_NB_BYTES);
                                Err(())
                            }
                        } else {
                            debug!(
                                "read REG_FIFO_RX_CURRENT_ADDR(0x{:02X}) error",
                                REG_FIFO_RX_CURRENT_ADDR
                            );
                            Err(())
                        }
                    }
                } else {
                    Err(())
                }
            } else {
                debug!("read REG_IRQ_FLAGS(0x{:02X}) error", REG_IRQ_FLAGS);
                Err(())
            }
        } else {
            debug!("set_mode_rx error");
            Err(())
        }
    }
}
