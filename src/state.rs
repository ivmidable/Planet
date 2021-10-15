use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Pluto {
    pub epoch: u64,
    pub epoch_start_block:u64,
    pub total_mined: u64,
    pub mined_this_epoch: u64,
    pub hash: [u8; 32],
    pub diff: u8,
    pub tokens: u64,
}

pub const PLUTO: Item<Pluto> = Item::new("work");