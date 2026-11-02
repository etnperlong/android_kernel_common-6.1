// SPDX-License-Identifier: GPL-2.0
use crate::{bindings, build_error};
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum DataDirection {
    Bidirectional = Self::const_cast(bindings::dma_data_direction_DMA_BIDIRECTIONAL),
    ToDevice = Self::const_cast(bindings::dma_data_direction_DMA_TO_DEVICE),
    FromDevice = Self::const_cast(bindings::dma_data_direction_DMA_FROM_DEVICE),
    None = Self::const_cast(bindings::dma_data_direction_DMA_NONE),
}
impl DataDirection {
    const fn const_cast(val: bindings::dma_data_direction) -> u32 {
        let wide_val = val as i128;
        if wide_val < 0 || wide_val > u32::MAX as i128 {
            build_error!("C enum value is out of bounds for the target type `u32`.");
        }
        wide_val as u32
    }
}
impl From<DataDirection> for bindings::dma_data_direction {
    fn from(direction: DataDirection) -> Self {
        direction as u32 as Self
    }
}
