// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use byteorder::{ByteOrder, LittleEndian};
use sys_util::GuestAddress;

use super::*;

/// Contains that data for reading and writing the common configuration structure of a virtio PCI
/// device.
///
/// * Registers:
/// ** About the whole device.
/// le32 device_feature_select;     // read-write
/// le32 device_feature;            // read-only for driver
/// le32 driver_feature_select;     // read-write
/// le32 driver_feature;            // read-write
/// le16 msix_config;               // read-write
/// le16 num_queues;                // read-only for driver
/// u8 device_status;               // read-write (driver_status)
/// u8 config_generation;           // read-only for driver
/// ** About a specific virtqueue.
/// le16 queue_select;              // read-write
/// le16 queue_size;                // read-write, power of 2, or 0.
/// le16 queue_msix_vector;         // read-write
/// le16 queue_enable;              // read-write (Ready)
/// le16 queue_notify_off;          // read-only for driver
/// le64 queue_desc;                // read-write
/// le64 queue_avail;               // read-write
/// le64 queue_used;                // read-write
pub struct VirtioPciCommonConfig {
    pub driver_status: u8,
    pub config_generation: u8,
    pub device_feature_select: u32,
    pub driver_feature_select: u32,
    pub queue_select: u16,
}

impl VirtioPciCommonConfig {
    pub fn read(
        &mut self,
        offset: u64,
        data: &mut [u8],
        queues: &mut Vec<Queue>,
        device: &mut Box<VirtioDevice>,
    ) {
        match data.len() {
            1 => {
                let v = self.read_common_config_byte(offset);
                data[0] = v;
            }
            2 => {
                let v = self.read_common_config_word(offset, queues);
                LittleEndian::write_u16(data, v);
            }
            4 => {
                let v = self.read_common_config_dword(offset, device);
                LittleEndian::write_u32(data, v);
            }
            8 => {
                let v = self.read_common_config_qword(offset);
                LittleEndian::write_u64(data, v);
            }
            _ => (),
        }
    }

    pub fn write(
        &mut self,
        offset: u64,
        data: &[u8],
        queues: &mut Vec<Queue>,
        device: &mut Box<VirtioDevice>,
    ) {
        match data.len() {
            1 => self.write_common_config_byte(offset, data[0]),
            2 => self.write_common_config_word(offset, LittleEndian::read_u16(data), queues),
            4 => {
                self.write_common_config_dword(offset, LittleEndian::read_u32(data), queues, device)
            }
            8 => self.write_common_config_qword(offset, LittleEndian::read_u64(data), queues),
            _ => (),
        }
    }

    fn read_common_config_byte(&self, offset: u64) -> u8 {
        // The driver is only allowed to do aligned, properly sized access.
        match offset {
            0x14 => self.driver_status,
            0x15 => self.config_generation,
            _ => 0,
        }
    }

    fn write_common_config_byte(&mut self, offset: u64, value: u8) {
        match offset {
            0x14 => self.driver_status = value,
            _ => {
                warn!("invalid virtio config byt access: 0x{:x}", offset);
            }
        }
    }

    fn read_common_config_word(&self, offset: u64, queues: &Vec<Queue>) -> u16 {
        match offset {
            0x10 => 0,                   // TODO msi-x: self.msix_config,
            0x12 => queues.len() as u16, // num_queues
            0x16 => self.queue_select,
            0x18 => self.with_queue(queues, |q| q.size).unwrap_or(0),
            0x1c => if self.with_queue(queues, |q| q.ready).unwrap_or(false) {
                1
            } else {
                0
            },
            0x1e => self.queue_select, // notify_off
            _ => 0,
        }
    }

    fn write_common_config_word(&mut self, offset: u64, value: u16, queues: &mut Vec<Queue>) {
        match offset {
            0x10 => (), // TODO msi-x: self.msix_config = value,
            0x16 => self.queue_select = value,
            0x18 => self.with_queue_mut(queues, |q| q.size = value),
            0x1a => (), // TODO msi-x: self.with_queue_mut(queues, |q| q.msix_vector = v),
            0x1c => self.with_queue_mut(queues, |q| q.ready = value == 1),
            _ => {
                warn!("invalid virtio register word write: 0x{:x}", offset);
            }
        }
    }

    fn read_common_config_dword(&self, offset: u64, device: &Box<VirtioDevice>) -> u32 {
        match offset {
            0x00 => self.device_feature_select,
            0x04 => device.features(self.device_feature_select),
            0x08 => self.driver_feature_select,
            _ => 0,
        }
    }

    fn write_common_config_dword(
        &mut self,
        offset: u64,
        value: u32,
        queues: &mut Vec<Queue>,
        device: &mut Box<VirtioDevice>,
    ) {
        fn hi(v: &mut GuestAddress, x: u32) {
            *v = (*v & 0xffffffff) | ((x as u64) << 32)
        }

        fn lo(v: &mut GuestAddress, x: u32) {
            *v = (*v & !0xffffffff) | (x as u64)
        }

        match offset {
            0x00 => self.device_feature_select = value,
            0x08 => self.driver_feature_select = value,
            0x0c => device.ack_features(self.device_feature_select, value),
            0x20 => self.with_queue_mut(queues, |q| lo(&mut q.desc_table, value)),
            0x24 => self.with_queue_mut(queues, |q| hi(&mut q.desc_table, value)),
            0x28 => self.with_queue_mut(queues, |q| lo(&mut q.avail_ring, value)),
            0x2c => self.with_queue_mut(queues, |q| hi(&mut q.avail_ring, value)),
            0x30 => self.with_queue_mut(queues, |q| lo(&mut q.used_ring, value)),
            0x34 => self.with_queue_mut(queues, |q| hi(&mut q.used_ring, value)),
            _ => {
                warn!("invalid virtio register dword write: 0x{:x}", offset);
            }
        }
    }

    fn read_common_config_qword(&self, offset: u64) -> u64 {
        0 // Assume the guest has no reason to read write-only registers.
    }

    fn write_common_config_qword(&mut self, offset: u64, value: u64, queues: &mut Vec<Queue>) {
        match offset {
            0x20 => self.with_queue_mut(queues, |q| q.desc_table = GuestAddress(value)),
            0x28 => self.with_queue_mut(queues, |q| q.avail_ring = GuestAddress(value)),
            0x30 => self.with_queue_mut(queues, |q| q.used_ring = GuestAddress(value)),
            _ => {
                warn!("invalid virtio register qword write: 0x{:x}", offset);
            }
        }
    }

    fn with_queue<U, F>(&self, queues: &Vec<Queue>, f: F) -> Option<U>
    where
        F: FnOnce(&Queue) -> U,
    {
        queues.get(self.queue_select as usize).map(f)
    }

    fn with_queue_mut<F: FnOnce(&mut Queue)>(&self, queues: &mut Vec<Queue>, f: F) {
        if let Some(queue) = queues.get_mut(self.queue_select as usize) {
            f(queue);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::os::unix::io::RawFd;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use sys_util::{EventFd, GuestMemory};

    struct DummyDevice(u32);
    const QUEUE_SIZE: u16 = 256;
    const QUEUE_SIZES: &'static [u16] = &[QUEUE_SIZE];
    const DUMMY_FEATURES: u32 = 0x5555_aaaa;
    impl VirtioDevice for DummyDevice {
        fn keep_fds(&self) -> Vec<RawFd> {
            Vec::new()
        }
        fn device_type(&self) -> u32 {
            return self.0;
        }
        fn queue_max_sizes(&self) -> &[u16] {
            QUEUE_SIZES
        }
        fn activate(&mut self,
                    _mem: GuestMemory,
                    _interrupt_evt: EventFd,
                    _status: Arc<AtomicUsize>,
                    _queues: Vec<Queue>,
                    _queue_evts: Vec<EventFd>) {
        }
        fn features(&self, page: u32) -> u32 {
            DUMMY_FEATURES
        }
    }

    #[test]
    fn write_base_regs() {
        let mut regs = VirtioPciCommonConfig {
            driver_status: 0xaa,
            config_generation: 0x55,
            device_feature_select: 0x0,
            driver_feature_select: 0x0,
            queue_select: 0xff,
        };

        let mut dev: Box<VirtioDevice> = Box::new(DummyDevice(0));
        let mut queues = Vec::new();

        // Can set all bits of driver_status.
        regs.write(0x14, &[0x55], &mut queues, &mut dev);
        let mut read_back = vec![0x00];
        regs.read(0x14, &mut read_back, &mut queues, &mut dev);
        assert_eq!(read_back[0], 0x55);

        // The config generation register is read only.
        regs.write(0x15, &[0xaa], &mut queues, &mut dev);
        let mut read_back = vec![0x00];
        regs.read(0x15, &mut read_back, &mut queues, &mut dev);
        assert_eq!(read_back[0], 0x55);

        // Device features is read-only and passed through from the device.
        regs.write(0x04, &[0, 0, 0, 0], &mut queues, &mut dev);
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x04, &mut read_back, &mut queues, &mut dev);
        assert_eq!(LittleEndian::read_u32(&read_back), DUMMY_FEATURES);

        // Feature select registers are read/write.
        regs.write(0x00, &[1, 2, 3, 4], &mut queues, &mut dev);
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x00, &mut read_back, &mut queues, &mut dev);
        assert_eq!(LittleEndian::read_u32(&read_back), 0x0403_0201);
        regs.write(0x08, &[1, 2, 3, 4], &mut queues, &mut dev);
        let mut read_back = vec![0, 0, 0, 0];
        regs.read(0x08, &mut read_back, &mut queues, &mut dev);
        assert_eq!(LittleEndian::read_u32(&read_back), 0x0403_0201);

        // 'queue_select' can be read and written.
        regs.write(0x16, &[0xaa, 0x55], &mut queues, &mut dev);
        let mut read_back = vec![0x00, 0x00];
        regs.read(0x16, &mut read_back, &mut queues, &mut dev);
        assert_eq!(read_back[0], 0xaa);
        assert_eq!(read_back[1], 0x55);
    }
}
