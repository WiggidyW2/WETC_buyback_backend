use crate::{Quantity, ItemName};

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub name: ItemName,
    pub quantity: Quantity,
}
