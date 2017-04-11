use hardware::Id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceConfig {
    pub id: Id,
    pub model: u32,
    pub interrupt: u32,
    pub memmap_base: u32,
    pub memmap_size: u32, // 0 = no memmap
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceModel {
    Ram = 0x01011010,
    Monitor = 0x384c0001,
    Keyboard = 0x384c000e,
    DebugConsole = 0xdeadbeef,
}

impl DeviceModel {
    pub fn number(self) -> u32 {
        self as u32
    }

    pub fn memory_size(self) -> Option<u32> {
        Some(match self {
            // Enough for 640x480 at 256 colors
            DeviceModel::Monitor => 0x14000,

            DeviceModel::Keyboard => 0x1,

            DeviceModel::DebugConsole => 0x3,

            _ => { return None; }
        })
    }
}
